//! Runtime support for the Nodarium node ABI v1, see `docs/ABI.md`.
//!
//! Every buffer handed to the host (inputs it allocated, results we produced)
//! stays alive until the host calls `nodarium_reset`, so the host can pass a
//! result pointer straight into the next node without copying it out first.
//!
//! A buffer the host allocates has [`HEADROOM`] spare words before and after it. A node may
//! return an input of the current call as its result with a few words written into that room,
//! see [`frame_input`], instead of copying it. A node that knows the size of a result before it
//! makes it writes it straight into a buffer of the ABI, see [`write_result`], instead of building
//! it elsewhere and copying it.
//!
//! A node that only passes large buffers on keeps these buffers in an [`Arena`] instead of the
//! allocator, see [`use_arena`]. There a node may also lengthen an input that is the last buffer
//! of its chunk, see [`extend_input`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};

/// Spare words before and after every buffer the host allocates
pub const HEADROOM: usize = 4;

/// Bytes in a page of wasm memory
pub const PAGE: usize = 65536;

/// Where a buffer lives: a vector of the allocator, or the address of words in the arena, which
/// `reset` rewinds
enum Storage {
    Heap(Vec<i32>),
    Arena(usize),
}

impl Storage {
    fn base(&mut self) -> *mut i32 {
        match self {
            Storage::Heap(buffer) => buffer.as_mut_ptr(),
            Storage::Arena(at) => *at as *mut i32,
        }
    }

    fn start(&self) -> *const i32 {
        match self {
            Storage::Heap(buffer) => buffer.as_ptr(),
            Storage::Arena(at) => *at as *const i32,
        }
    }
}

/// A buffer that lives until the next reset. `input` is the number of words the host asked for,
/// for the buffers it allocated for the current call that weren't framed yet.
struct Allocation {
    storage: Storage,
    input: Option<usize>,
}

impl Allocation {
    /// Where the words the host asked for start, after the headroom
    fn input_start(&self) -> *const i32 {
        // SAFETY: inside the buffer, see `alloc`
        unsafe { self.storage.start().add(HEADROOM) }
    }
}

static ALLOCATIONS: Mutex<Vec<Allocation>> = Mutex::new(Vec::new());

// Result descriptor returned by `nodarium_execute`: [ptr, len] per output
static RESULT: Mutex<Vec<u32>> = Mutex::new(Vec::new());

static USE_ARENA: AtomicBool = AtomicBool::new(false);
static ARENA: Mutex<Arena> = Mutex::new(Arena { chunks: Vec::new() });

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn nodarium_panic(ptr: *const u8, len: usize);
}

/// The lists of allocations and results outlive `reset`. Reserved before the first buffer, they
/// sit below all buffers. Grown later, they would land between large buffers and split the
/// memory freed by `reset`, so a later, larger run couldn't reuse it and would grow the memory.
fn reserve_lists() {
    static RESERVED: Once = Once::new();
    RESERVED.call_once(|| {
        ALLOCATIONS.lock().unwrap().reserve(1024);
        RESULT.lock().unwrap().reserve(64);
        ARENA.lock().unwrap().chunks.reserve(16);
    });
}

/// Keeps the buffers the host allocates and the results written in place in the [`Arena`] from
/// now on, for a node that only passes large buffers on, see `#[nodarium_execute(arena)]`
pub fn use_arena() {
    USE_ARENA.store(true, Ordering::Relaxed);
}

/// The memory a module grows, by whole pages
trait Memory {
    /// Where the memory ends, in bytes
    fn end(&self) -> usize;
    /// Grows the memory by `pages`, returns where the new ones start
    fn grow(&mut self, pages: usize) -> usize;
}

#[cfg(target_arch = "wasm32")]
struct ModuleMemory;

#[cfg(target_arch = "wasm32")]
impl Memory for ModuleMemory {
    fn end(&self) -> usize {
        core::arch::wasm32::memory_size(0) * PAGE
    }

    fn grow(&mut self, pages: usize) -> usize {
        let old = core::arch::wasm32::memory_grow(0, pages);
        assert!(old != usize::MAX, "out of memory: can't grow by {pages} pages");
        old * PAGE
    }
}

/// Off wasm, for the tests, the memory is a region reserved once that grows inside it
#[cfg(not(target_arch = "wasm32"))]
struct ModuleMemory;

#[cfg(not(target_arch = "wasm32"))]
impl ModuleMemory {
    const PAGES: usize = 256;

    fn base() -> usize {
        static BASE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        let layout = std::alloc::Layout::from_size_align(Self::PAGES * PAGE, PAGE).unwrap();
        // SAFETY: the layout isn't empty
        *BASE.get_or_init(|| unsafe { std::alloc::alloc(layout) } as usize)
    }
}

#[cfg(not(target_arch = "wasm32"))]
static GROWN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(not(target_arch = "wasm32"))]
impl Memory for ModuleMemory {
    fn end(&self) -> usize {
        Self::base() + GROWN.load(Ordering::Relaxed) * PAGE
    }

    fn grow(&mut self, pages: usize) -> usize {
        let old = GROWN.fetch_add(pages, Ordering::Relaxed);
        assert!(old + pages <= Self::PAGES, "out of memory: can't grow by {pages} pages");
        Self::base() + old * PAGE
    }
}

/// A region at the end of the memory for buffers that live until the reset, which rewinds it.
///
/// After a reset the allocator holds one free block of the size of the last run. When the input
/// of a slightly larger run still fits into it but the output doesn't, the allocator grows the
/// memory by the whole output, so the memory of a node that passes a mesh on grows by a mesh
/// over builds. The arena grows by the pages it misses: buffers are laid one after the other, and
/// when they don't fit any more the last chunk grows where the memory ends. Only when the
/// allocator grew the memory after it does the arena start another chunk there.
///
/// The allocator never hands out the arena's pages: it only extends its memory by pages that start
/// where it ends, see `docs/ABI.md` and `app/src/lib/abi-arena.test.ts`.
struct Arena {
    chunks: Vec<Chunk>,
}

/// Bytes of the memory from `start` to `end`, used up to `top`
struct Chunk {
    start: usize,
    end: usize,
    top: usize,
}

impl Arena {
    /// The address of `bytes` that stay valid until [`Arena::rewind`]
    fn alloc(&mut self, bytes: usize, memory: &mut impl Memory) -> usize {
        let bytes = bytes.next_multiple_of(8);
        if let Some(chunk) = self.chunks.iter_mut().find(|c| c.end - c.top >= bytes) {
            chunk.top += bytes;
            return chunk.top - bytes;
        }
        let end = memory.end();
        match self.chunks.last_mut() {
            Some(last) if last.end == end => {
                let pages = (bytes - (last.end - last.top)).div_ceil(PAGE);
                memory.grow(pages);
                last.end += pages * PAGE;
                last.top += bytes;
                last.top - bytes
            }
            _ => {
                let pages = bytes.div_ceil(PAGE);
                let start = memory.grow(pages);
                self.chunks.push(Chunk { start, end: start + pages * PAGE, top: start + bytes });
                start
            }
        }
    }

    /// Lengthens the `bytes` at `at` to `to` bytes when they are the last of their chunk: inside
    /// the chunk, or by the missing pages when the chunk ends where the memory does
    fn extend(&mut self, at: usize, bytes: usize, to: usize, memory: &mut impl Memory) -> bool {
        let (bytes, to) = (bytes.next_multiple_of(8), to.next_multiple_of(8));
        let end = memory.end();
        let Some(chunk) = self.chunks.iter_mut().find(|c| c.start <= at && at + bytes == c.top) else {
            return false;
        };
        let missing = (at + to).saturating_sub(chunk.end);
        if missing > 0 {
            if chunk.end != end {
                return false;
            }
            let pages = missing.div_ceil(PAGE);
            memory.grow(pages);
            chunk.end += pages * PAGE;
        }
        chunk.top = at + to;
        true
    }

    fn rewind(&mut self) {
        for chunk in &mut self.chunks {
            chunk.top = chunk.start;
        }
    }
}

/// A buffer of `words` that lives until the reset, zeroed when `zero`. Buffers smaller than a
/// page stay out of the arena, so the large ones lie end to end there and the host's small
/// buffers after an input, its other arguments and the argument table, don't keep it from
/// being extended.
fn buffer(words: usize, zero: bool) -> Storage {
    if !USE_ARENA.load(Ordering::Relaxed) || words * 4 < PAGE {
        return Storage::Heap(if zero { vec![0; words] } else { Vec::with_capacity(words) });
    }
    let at = ARENA.lock().unwrap().alloc(words * 4, &mut ModuleMemory);
    if zero {
        // SAFETY: the arena gave out these words, nothing else refers to them
        unsafe { std::ptr::write_bytes(at as *mut i32, 0, words) };
    }
    Storage::Arena(at)
}

/// Allocates `bytes` of 4-byte aligned memory for the host that stays valid until `reset`, with
/// [`HEADROOM`] spare words on either side.
pub fn alloc(bytes: usize) -> *mut i32 {
    reserve_lists();
    let words = bytes.div_ceil(4);
    let mut storage = buffer(words + 2 * HEADROOM, false);
    // SAFETY: the buffer holds the headroom before the words
    let ptr = unsafe { storage.base().add(HEADROOM) };
    ALLOCATIONS.lock().unwrap().push(Allocation { storage, input: Some(words) });
    ptr
}

/// Frees every allocation and result since the last reset.
pub fn reset() {
    ALLOCATIONS.lock().unwrap().clear();
    ARENA.lock().unwrap().rewind();
}

/// Words of a buffer the ABI owns until the next reset, returned as a result without copying
/// them: an input framed in place, see [`frame_input`], or a result written in place, see
/// [`write_result`]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InPlace {
    ptr: *const i32,
    len: usize,
}

impl InPlace {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The words
    ///
    /// # Safety
    /// Only until the next `reset`, which frees them.
    pub unsafe fn words(&self) -> &[i32] {
        std::slice::from_raw_parts(self.ptr, self.len)
    }

    /// The words, to change them before they are returned
    ///
    /// # Safety
    /// Only before it is returned, and no other reference to the words may be used meanwhile:
    /// for an input given back by [`frame_input`] or [`extend_input`], the input isn't read again.
    pub unsafe fn words_mut(&mut self) -> &mut [i32] {
        std::slice::from_raw_parts_mut(self.ptr as *mut i32, self.len)
    }
}

/// `prefix`, `input` and `suffix` as one result, without copying `input`: the words are written into
/// the headroom of the buffer the host allocated for it. `None` when `input` isn't a whole buffer the
/// host allocated for the current call, such as a result of this module that other nodes may still
/// read, or when `prefix` or `suffix` are longer than [`HEADROOM`]; then the node copies it.
///
/// The host never reads an input again once the node ran, see `docs/ABI.md`, so the node may write
/// around it. Every input can be framed once.
pub fn frame_input(input: &[i32], prefix: &[i32], suffix: &[i32]) -> Option<InPlace> {
    if input.is_empty() || prefix.len() > HEADROOM || suffix.len() > HEADROOM {
        return None;
    }
    let mut allocations = ALLOCATIONS.lock().unwrap();
    let allocation =
        allocations.iter_mut().rev().find(|a| a.input == Some(input.len()) && a.input_start() == input.as_ptr())?;
    allocation.input = Some(0);
    let base = allocation.storage.base();
    let first = HEADROOM - prefix.len();
    // SAFETY: the prefix ends where the input starts and the suffix starts where it ends, both in the
    // headroom of the buffer, which no reference to the input covers
    unsafe {
        std::ptr::copy_nonoverlapping(prefix.as_ptr(), base.add(first), prefix.len());
        std::ptr::copy_nonoverlapping(suffix.as_ptr(), base.add(HEADROOM + input.len()), suffix.len());
    }
    Some(InPlace { ptr: unsafe { base.add(first) }, len: prefix.len() + input.len() + suffix.len() })
}

/// `input` and `words` zeroed words after it as one result, without copying `input`, for a node
/// that returns its input with more words. `None` when `input` isn't a whole buffer the host
/// allocated for the current call, see [`frame_input`], when it isn't in the arena or isn't the
/// last buffer of its chunk, or when the memory grew after that chunk; then the node copies it.
/// An extended input can't be framed, shrunk or extended again.
pub fn extend_input(input: &[i32], words: usize) -> Option<InPlace> {
    if input.is_empty() {
        return None;
    }
    let mut allocations = ALLOCATIONS.lock().unwrap();
    let allocation =
        allocations.iter_mut().rev().find(|a| a.input == Some(input.len()) && a.input_start() == input.as_ptr())?;
    let Storage::Arena(at) = allocation.storage else {
        return None;
    };
    let bytes = (input.len() + 2 * HEADROOM) * 4;
    if !ARENA.lock().unwrap().extend(at, bytes, bytes + words * 4, &mut ModuleMemory) {
        return None;
    }
    allocation.input = None;
    let ptr = (at as *mut i32).wrapping_add(HEADROOM);
    // SAFETY: the arena gave out these words after the input, nothing else refers to them
    unsafe { std::ptr::write_bytes(ptr.add(input.len()), 0, words) };
    Some(InPlace { ptr, len: input.len() + words })
}

/// A result written straight into a buffer of `words` that the ABI owns until the next reset.
/// `write` gets the buffer zeroed and returns how many of its first words are the result, so a
/// node that knows only an upper bound of its size returns fewer. The rest stays unused until the
/// reset.
pub fn write_result(words: usize, write: impl FnOnce(&mut [i32]) -> usize) -> InPlace {
    reserve_lists();
    let mut storage = buffer(words, true);
    let ptr = storage.base();
    // SAFETY: the buffer holds `words` zeroed words that nothing else refers to
    let len = write(unsafe { std::slice::from_raw_parts_mut(ptr, words) });
    assert!(len <= words, "a result of {len} words in a buffer of {words}");
    ALLOCATIONS.lock().unwrap().push(Allocation { storage, input: None });
    InPlace { ptr, len }
}

/// Gives back the memory of `input` after its first `keep` words, for a node that copied the
/// rest out and only reads those words again: a decoded input would otherwise stay in memory next
/// to its copy until the reset. Returns the words that are kept, which may have moved, and `None`
/// when `input` isn't a whole buffer the host allocated for the current call, see [`frame_input`],
/// or is in the arena; then nothing changes. A shrunk input can't be framed.
///
/// # Safety
/// When it returns the kept words, `input` must not be read again.
pub unsafe fn shrink_input<'a>(input: &'a [i32], keep: usize) -> Option<&'a [i32]> {
    let mut allocations = ALLOCATIONS.lock().unwrap();
    let allocation =
        allocations.iter_mut().find(|a| a.input == Some(input.len()) && a.input_start() == input.as_ptr())?;
    let Storage::Heap(buffer) = &mut allocation.storage else {
        return None;
    };
    let keep = keep.min(input.len());
    buffer.shrink_to(HEADROOM + keep);
    allocation.input = None;
    Some(std::slice::from_raw_parts(allocation.input_start(), keep))
}

/// Reads the argument table `[ptr0, len0, ptr1, len1, ...]` written by the host.
///
/// # Safety
/// `args_ptr` must point to `argc` (ptr, len) pairs describing valid i32 slices
/// that outlive the returned references.
pub unsafe fn read_args<'a>(args_ptr: *const u32, argc: usize) -> Vec<&'a [i32]> {
    if argc == 0 {
        return Vec::new();
    }
    let table = std::slice::from_raw_parts(args_ptr, argc * 2);
    table
        .chunks_exact(2)
        .map(|pair| {
            if pair[1] == 0 {
                &[][..]
            } else {
                std::slice::from_raw_parts(pair[0] as *const i32, pair[1] as usize)
            }
        })
        .collect()
}

/// One output of a node: words of its own, or words the ABI already owns, see [`InPlace`]
#[derive(Debug, PartialEq, Eq)]
pub enum Output {
    Words(Vec<i32>),
    InPlace(InPlace),
}

impl Output {
    /// The words of the output
    ///
    /// # Safety
    /// An output in place only until the next `reset`, see [`InPlace::words`].
    pub unsafe fn words(&self) -> &[i32] {
        match self {
            Output::Words(words) => words,
            Output::InPlace(in_place) => in_place.words(),
        }
    }
}

impl From<Vec<i32>> for Output {
    fn from(words: Vec<i32>) -> Output {
        Output::Words(words)
    }
}

impl From<InPlace> for Output {
    fn from(in_place: InPlace) -> Output {
        Output::InPlace(in_place)
    }
}

/// What an execute function can return: one value per output of the node.
///
/// A single `Vec<i32>` or [`Output`] is a node with one output. Several outputs
/// are returned as a `Vec<Vec<i32>>`, an array or a tuple, in the order of
/// `outputs` in the node definition.
pub trait NodeOutputs {
    fn into_outputs(self) -> Vec<Output>;
}

impl NodeOutputs for Vec<i32> {
    fn into_outputs(self) -> Vec<Output> {
        vec![self.into()]
    }
}

impl NodeOutputs for Output {
    fn into_outputs(self) -> Vec<Output> {
        vec![self]
    }
}

impl NodeOutputs for Vec<Vec<i32>> {
    fn into_outputs(self) -> Vec<Output> {
        self.into_iter().map(Output::from).collect()
    }
}

impl<const N: usize> NodeOutputs for [Vec<i32>; N] {
    fn into_outputs(self) -> Vec<Output> {
        self.into_iter().map(Output::from).collect()
    }
}

macro_rules! impl_node_outputs_for_tuple {
    ($($name:ident),+) => {
        impl NodeOutputs for ($(impl_node_outputs_for_tuple!(@vec $name),)+) {
            #[allow(non_snake_case)]
            fn into_outputs(self) -> Vec<Output> {
                let ($($name,)+) = self;
                vec![$($name.into()),+]
            }
        }
    };
    (@vec $name:ident) => { Vec<i32> };
}

impl_node_outputs_for_tuple!(A, B);
impl_node_outputs_for_tuple!(A, B, C);
impl_node_outputs_for_tuple!(A, B, C, D);
impl_node_outputs_for_tuple!(A, B, C, D, E);
impl_node_outputs_for_tuple!(A, B, C, D, E, F);
impl_node_outputs_for_tuple!(A, B, C, D, E, F, G);
impl_node_outputs_for_tuple!(A, B, C, D, E, F, G, H);

/// Keeps every output alive until the next reset and returns a pointer to the
/// `[ptr, len]` pairs describing them, one pair per output. The inputs of the
/// call can no longer be framed.
pub fn store_result(result: impl NodeOutputs) -> *const u32 {
    reserve_lists();
    let outputs = result.into_outputs();
    let mut descriptor = RESULT.lock().unwrap();
    descriptor.clear();
    let mut allocations = ALLOCATIONS.lock().unwrap();
    // results written in place lie between the inputs of a call, so every allocation is looked at
    for allocation in allocations.iter_mut() {
        allocation.input = None;
    }
    for output in outputs {
        match output {
            Output::Words(buffer) => {
                descriptor.push(buffer.as_ptr() as u32);
                descriptor.push(buffer.len() as u32);
                allocations.push(Allocation { storage: Storage::Heap(buffer), input: None });
            }
            // the buffer is one of the allocations already
            Output::InPlace(in_place) => {
                descriptor.push(in_place.ptr as u32);
                descriptor.push(in_place.len as u32);
            }
        }
    }
    descriptor.as_ptr()
}

pub fn setup_panic_hook() {
    static SET_HOOK: std::sync::Once = std::sync::Once::new();
    SET_HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let msg = info.to_string();
            #[cfg(target_arch = "wasm32")]
            unsafe {
                nodarium_panic(msg.as_ptr(), msg.len());
            }
            #[cfg(not(target_arch = "wasm32"))]
            eprintln!("{}", msg);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The allocations are global, tests that use them take turns
    static STATE: Mutex<()> = Mutex::new(());

    fn words(outputs: Vec<Output>) -> Vec<Vec<i32>> {
        outputs.iter().map(|o| unsafe { o.words() }.to_vec()).collect()
    }

    /// What the host does: allocates a buffer and writes `words` into it
    fn host_input(words: &[i32]) -> &'static [i32] {
        let ptr = alloc(words.len() * 4);
        unsafe {
            std::ptr::copy_nonoverlapping(words.as_ptr(), ptr, words.len());
            std::slice::from_raw_parts(ptr, words.len())
        }
    }

    #[test]
    fn an_input_of_the_call_is_framed_in_place() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        let input = host_input(&[7, 8, 9]);
        let table = host_input(&[0, 3]);
        let framed = frame_input(input, &[0, 1, 0, 4], &[1, 1, 1, 1]).unwrap();
        assert_eq!(unsafe { framed.words() }, &[0, 1, 0, 4, 7, 8, 9, 1, 1, 1, 1]);
        // in place: the input words are the framed ones
        assert_eq!(unsafe { framed.words() }[4..7].as_ptr(), input.as_ptr());
        // once only, and the table around it is untouched
        assert_eq!(frame_input(input, &[], &[]), None);
        assert_eq!(table, &[0, 3]);
        let descriptor = store_result(Output::from(framed));
        let pair = unsafe { std::slice::from_raw_parts(descriptor, 2) };
        assert_eq!(pair, &[framed.ptr as u32, 11]);
        reset();
    }

    #[test]
    fn only_whole_inputs_of_the_call_are_framed() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        let input = host_input(&[1, 2, 3, 4]);
        // a part of an input, words that aren't an input, more than the headroom
        assert_eq!(frame_input(&input[1..], &[0], &[]), None);
        assert_eq!(frame_input(&input[..3], &[0], &[]), None);
        assert_eq!(frame_input(&[1, 2, 3, 4], &[0], &[]), None);
        assert_eq!(frame_input(input, &[0; HEADROOM + 1], &[]), None);
        assert_eq!(frame_input(input, &[], &[0; HEADROOM + 1]), None);
        // the result of a call, passed into the next one, may still be read by other nodes
        store_result(vec![5, 6, 7]);
        let (ptr, len) = {
            let allocations = ALLOCATIONS.lock().unwrap();
            let Storage::Heap(result) = &allocations.last().unwrap().storage else { unreachable!() };
            (result.as_ptr(), result.len())
        };
        let result = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert_eq!(frame_input(result, &[0], &[]), None);
        // nor an input of a call that is over
        assert_eq!(frame_input(input, &[0], &[]), None);
        let next = host_input(&[1, 2]);
        assert!(frame_input(next, &[0], &[9]).is_some());
        reset();
        assert_eq!(frame_input(next, &[0], &[9]), None);
    }

    #[test]
    fn a_result_is_written_in_place() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        let input = host_input(&[1, 2, 3]);
        let result = write_result(5, |words| {
            assert_eq!(words, &[0; 5]);
            words[..3].copy_from_slice(&[4, 5, 6]);
            // the inputs of the call can still be framed while a result is written
            assert!(frame_input(input, &[0], &[]).is_some());
            3
        });
        assert_eq!(unsafe { result.words() }, &[4, 5, 6]);
        let descriptor = store_result(Output::from(result));
        let pair = unsafe { std::slice::from_raw_parts(descriptor, 2) };
        assert_eq!(pair, &[result.ptr as u32, 3]);
        // the result stays until the reset, and after the call no input can be framed
        let next = host_input(&[7, 8]);
        let other = write_result(1, |words| {
            words[0] = 9;
            1
        });
        store_result(Output::from(other));
        assert_eq!(unsafe { result.words() }, &[4, 5, 6]);
        assert_eq!(frame_input(next, &[0], &[]), None);
        assert_eq!(frame_input(input, &[], &[]), None);
        reset();
    }

    #[test]
    fn an_input_of_the_call_is_shrunk_to_what_is_kept() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        let input = host_input(&[1, 2, 3, 4, 5]);
        let other = host_input(&[6, 7]);
        // nor a part of it, or words that aren't an input
        assert_eq!(unsafe { shrink_input(&input[1..], 2) }, None);
        assert_eq!(unsafe { shrink_input(&[1, 2, 3, 4, 5], 2) }, None);
        let kept = unsafe { shrink_input(input, 2) }.unwrap();
        assert_eq!(kept, &[1, 2]);
        let capacity = {
            let allocations = ALLOCATIONS.lock().unwrap();
            allocations
                .iter()
                .map(|a| match &a.storage {
                    Storage::Heap(buffer) => buffer.capacity(),
                    Storage::Arena(_) => unreachable!(),
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(capacity, vec![HEADROOM + 2, 2 + 2 * HEADROOM]);
        // once only, and it can't be framed any more, the other input still can
        assert_eq!(unsafe { shrink_input(kept, 1) }, None);
        assert_eq!(frame_input(kept, &[], &[]), None);
        assert!(frame_input(other, &[0], &[]).is_some());
        store_result(vec![0]);
        let next = host_input(&[8, 9]);
        store_result(vec![0]);
        // nor an input of a call that is over
        assert_eq!(unsafe { shrink_input(next, 1) }, None);
        reset();
    }

    /// A memory that only counts its pages, and grows when something else asks for it
    struct Pages(usize);

    impl Memory for Pages {
        fn end(&self) -> usize {
            self.0 * PAGE
        }

        fn grow(&mut self, pages: usize) -> usize {
            self.0 += pages;
            (self.0 - pages) * PAGE
        }
    }

    #[test]
    fn the_arena_grows_by_the_pages_it_misses() {
        let mut memory = Pages(3);
        let mut arena = Arena { chunks: Vec::new() };
        // an input and the output of a run
        assert_eq!(arena.alloc(40 * PAGE + 12, &mut memory), 3 * PAGE);
        assert_eq!(arena.alloc(40 * PAGE, &mut memory), 43 * PAGE + 16);
        assert_eq!(memory.0, 84);
        // a slightly larger run grows by what it misses, not by its output
        arena.rewind();
        assert_eq!(arena.alloc(41 * PAGE, &mut memory), 3 * PAGE);
        assert_eq!(arena.alloc(41 * PAGE, &mut memory), 44 * PAGE);
        assert_eq!(memory.0, 85);
        // a smaller run fits
        arena.rewind();
        assert_eq!(arena.alloc(30 * PAGE, &mut memory), 3 * PAGE);
        assert_eq!(arena.alloc(30 * PAGE, &mut memory), 33 * PAGE);
        assert_eq!(memory.0, 85);
    }

    #[test]
    fn the_arena_starts_a_chunk_after_memory_it_does_not_own() {
        let mut memory = Pages(1);
        let mut arena = Arena { chunks: Vec::new() };
        assert_eq!(arena.alloc(2 * PAGE, &mut memory), PAGE);
        // the allocator grows the memory after the arena
        memory.grow(1);
        assert_eq!(arena.alloc(3 * PAGE, &mut memory), 4 * PAGE);
        assert_eq!(arena.chunks.len(), 2);
        // what fits goes into the first chunk again
        arena.rewind();
        assert_eq!(arena.alloc(PAGE, &mut memory), PAGE);
        assert_eq!(arena.alloc(2 * PAGE, &mut memory), 4 * PAGE);
        assert_eq!(arena.alloc(PAGE, &mut memory), 2 * PAGE);
        assert_eq!(arena.alloc(8, &mut memory), 6 * PAGE);
        assert_eq!(memory.0, 7);
    }

    /// Words of a buffer that is large enough for the arena
    const LARGE: usize = PAGE / 4;

    fn large(first: i32) -> Vec<i32> {
        (first..first + LARGE as i32).collect()
    }

    fn in_arena(words: &[i32]) -> bool {
        let allocations = ALLOCATIONS.lock().unwrap();
        let allocation =
            allocations.iter().find(|a| a.input_start() == words.as_ptr() || a.storage.start() == words.as_ptr());
        matches!(allocation.unwrap().storage, Storage::Arena(_))
    }

    #[test]
    fn inputs_and_results_in_the_arena_are_framed_written_and_rewound() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        use_arena();
        let input = host_input(&large(7));
        assert!(in_arena(input));
        let framed = frame_input(input, &[0, 4], &[1]).unwrap();
        assert_eq!(unsafe { framed.words() }[..4], [0, 4, 7, 8]);
        assert_eq!(unsafe { framed.words() }[LARGE + 2..], [1]);
        // memory used before is zeroed for a result
        let result = write_result(LARGE, |words| {
            assert!(words.iter().all(|&w| w == 0));
            words.fill(5);
            LARGE
        });
        assert_eq!(unsafe { shrink_input(input, 1) }, None);
        store_result((vec![0], vec![1]));
        reset();
        // a reset rewinds the arena, the next run uses the same memory
        assert_eq!(host_input(&large(1)).as_ptr(), input.as_ptr());
        let again = write_result(LARGE, |words| {
            assert!(words.iter().all(|&w| w == 0));
            0
        });
        assert_eq!(again.ptr, result.ptr);
        USE_ARENA.store(false, Ordering::Relaxed);
        reset();
    }

    #[test]
    fn buffers_smaller_than_a_page_stay_out_of_the_arena() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        use_arena();
        let mesh = host_input(&large(0));
        let color = host_input(&[1, 2, 3]);
        let table = host_input(&[0, 0, 0, 0]);
        let small = write_result(4, |_| 4);
        assert!(in_arena(mesh));
        assert!(!in_arena(color) && !in_arena(table));
        assert!(!in_arena(unsafe { small.words() }));
        // the large ones lie end to end
        let next = host_input(&large(0));
        let bytes = ((LARGE + 2 * HEADROOM) * 4).next_multiple_of(8);
        assert_eq!(next.as_ptr() as usize - mesh.as_ptr() as usize, bytes);
        USE_ARENA.store(false, Ordering::Relaxed);
        reset();
    }

    #[test]
    fn the_last_input_of_its_chunk_is_extended() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        use_arena();
        let input = host_input(&large(3));
        // the host's small buffers after it don't count
        let color = host_input(&[1, 2, 3]);
        host_input(&[0, 0, 0, 0]);
        // by more than is left of the page, the memory grows by the pages that are missing
        let words = PAGE / 4 + 5;
        let mut extended = extend_input(input, words).unwrap();
        assert_eq!(extended.len(), LARGE + words);
        assert_eq!(unsafe { extended.words() }.as_ptr(), input.as_ptr());
        assert_eq!(unsafe { extended.words() }[..LARGE], large(3)[..]);
        assert!(unsafe { extended.words() }[LARGE..].iter().all(|&w| w == 0));
        unsafe { extended.words_mut()[LARGE + words - 1] = 9 };
        // once only, and it can't be framed or shrunk any more
        assert_eq!(extend_input(input, 1), None);
        assert_eq!(frame_input(input, &[], &[]), None);
        assert_eq!(unsafe { shrink_input(input, 1) }, None);
        // the next buffer of the arena starts after it and its headroom
        let result = write_result(LARGE, |_| LARGE);
        let end = input.as_ptr() as usize + (LARGE + words + HEADROOM) * 4;
        assert!(result.ptr as usize >= end);
        assert_eq!(color, &[1, 2, 3]);
        let descriptor = store_result(Output::from(extended));
        let pair = unsafe { std::slice::from_raw_parts(descriptor, 2) };
        assert_eq!(pair, &[input.as_ptr() as u32, (LARGE + words) as u32]);
        assert_eq!(unsafe { extended.words() }[LARGE + words - 1], 9);
        USE_ARENA.store(false, Ordering::Relaxed);
        reset();
    }

    #[test]
    fn only_the_last_whole_input_of_the_call_in_its_chunk_is_extended() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        // not in the arena
        let input = host_input(&large(0));
        assert_eq!(extend_input(input, 1), None);
        reset();
        use_arena();
        // small inputs aren't in the arena either
        assert_eq!(extend_input(host_input(&[1, 2]), 1), None);
        let first = host_input(&large(0));
        let last = host_input(&large(1));
        // an input with a large buffer after it, a part of an input, words that aren't an input
        assert_eq!(extend_input(first, 1), None);
        assert_eq!(extend_input(&last[1..], 1), None);
        assert_eq!(extend_input(&last[..LARGE - 1], 1), None);
        assert_eq!(extend_input(&large(1), 1), None);
        // a framed input
        assert!(frame_input(last, &[], &[]).is_some());
        assert_eq!(extend_input(last, 1), None);
        // the result of a call, passed into the next one, may still be read by other nodes
        let result = write_result(LARGE, |_| LARGE);
        store_result(Output::from(result));
        assert_eq!(extend_input(unsafe { result.words() }, 1), None);
        // nor an input of a call that is over
        let next = host_input(&large(2));
        store_result(vec![0]);
        assert_eq!(extend_input(next, 1), None);
        USE_ARENA.store(false, Ordering::Relaxed);
        reset();
    }

    #[test]
    fn the_arena_extends_only_the_last_buffer_of_a_chunk() {
        let mut memory = Pages(1);
        let mut arena = Arena { chunks: Vec::new() };
        let first = arena.alloc(PAGE / 2, &mut memory);
        let last = arena.alloc(PAGE / 4 + 4, &mut memory);
        assert!(!arena.extend(first, PAGE / 2, PAGE / 2 + 8, &mut memory));
        // inside the chunk, then by the pages that are missing
        assert!(arena.extend(last, PAGE / 4 + 4, PAGE / 4 + 12, &mut memory));
        assert_eq!(memory.0, 2);
        assert!(arena.extend(last, PAGE / 4 + 12, 3 * PAGE, &mut memory));
        assert_eq!(memory.0, 5);
        assert_eq!(arena.alloc(8, &mut memory), last + 3 * PAGE);
        // not when the memory grew after the chunk, unless it fits
        let top = arena.alloc(PAGE / 4, &mut memory);
        memory.grow(1);
        assert!(!arena.extend(top, PAGE / 4, PAGE, &mut memory));
        assert!(arena.extend(top, PAGE / 4, PAGE / 4 + 16, &mut memory));
        assert_eq!(memory.0, 6);
    }

    #[test]
    #[should_panic(expected = "a result of 3 words in a buffer of 2")]
    fn a_result_fits_its_buffer() {
        write_result(2, |_| 3);
    }

    #[test]
    fn test_args_roundtrip() {
        let a = [1, 2, 3];
        let b = [4];
        let table = [a.as_ptr() as u32, 3, b.as_ptr() as u32, 1, 0, 0];
        // host pointers are 32 bit, so this only holds on wasm32
        if std::mem::size_of::<usize>() == 4 {
            let args = unsafe { read_args(table.as_ptr(), 3) };
            assert_eq!(args, vec![&a[..], &b[..], &[][..]]);
        }
        assert!(unsafe { read_args(std::ptr::null(), 0) }.is_empty());
    }

    #[test]
    fn test_outputs() {
        assert_eq!(words(vec![1, 2].into_outputs()), vec![vec![1, 2]]);
        assert_eq!(words((vec![1], vec![2, 3]).into_outputs()), vec![vec![1], vec![2, 3]]);
        assert_eq!(words([vec![1], vec![2]].into_outputs()), vec![vec![1], vec![2]]);
        assert_eq!(vec![vec![1], vec![], vec![3]].into_outputs().len(), 3);
    }

    #[test]
    fn test_store_result_writes_one_pair_per_output() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let a = vec![1, 2, 3];
        let b = vec![4];
        let (a_ptr, b_ptr) = (a.as_ptr() as u32, b.as_ptr() as u32);
        let descriptor = store_result((a, b));
        let pairs = unsafe { std::slice::from_raw_parts(descriptor, 4) };
        assert_eq!(pairs, &[a_ptr, 3, b_ptr, 1]);
        reset();
    }
}
