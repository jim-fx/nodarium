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
//! allocator, see [`use_arena`].

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

    fn rewind(&mut self) {
        for chunk in &mut self.chunks {
            chunk.top = chunk.start;
        }
    }
}

/// A buffer of `words` that lives until the reset, zeroed when `zero`
fn buffer(words: usize, zero: bool) -> Storage {
    if !USE_ARENA.load(Ordering::Relaxed) {
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

    #[test]
    fn inputs_and_results_in_the_arena_are_framed_written_and_rewound() {
        let _turn = STATE.lock().unwrap_or_else(|e| e.into_inner());
        reset();
        use_arena();
        let input = host_input(&[7, 8, 9]);
        let framed = frame_input(input, &[0, 4], &[1]).unwrap();
        assert_eq!(unsafe { framed.words() }, &[0, 4, 7, 8, 9, 1]);
        // memory used before is zeroed for a result
        let result = write_result(3, |words| {
            assert_eq!(words, &[0; 3]);
            words.copy_from_slice(&[5, 5, 5]);
            3
        });
        assert_eq!(unsafe { shrink_input(input, 1) }, None);
        store_result((vec![0], vec![1]));
        reset();
        // a reset rewinds the arena, the next run uses the same memory
        assert_eq!(host_input(&[1, 2, 3]).as_ptr(), input.as_ptr());
        let again = write_result(3, |words| {
            assert_eq!(words, &[0; 3]);
            0
        });
        assert_eq!(again.ptr, result.ptr);
        USE_ARENA.store(false, Ordering::Relaxed);
        reset();
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
