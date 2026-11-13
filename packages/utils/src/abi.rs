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

use std::sync::{Mutex, Once};

/// Spare words before and after every buffer the host allocates
pub const HEADROOM: usize = 4;

/// A buffer that lives until the next reset. `input` is the number of words the host asked for,
/// for the buffers it allocated for the current call that weren't framed yet.
struct Allocation {
    buffer: Vec<i32>,
    input: Option<usize>,
}

static ALLOCATIONS: Mutex<Vec<Allocation>> = Mutex::new(Vec::new());

// Result descriptor returned by `nodarium_execute`: [ptr, len] per output
static RESULT: Mutex<Vec<u32>> = Mutex::new(Vec::new());

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
    });
}

/// Allocates `bytes` of 4-byte aligned memory for the host that stays valid until `reset`, with
/// [`HEADROOM`] spare words on either side.
pub fn alloc(bytes: usize) -> *mut i32 {
    reserve_lists();
    let words = bytes.div_ceil(4);
    let mut buffer: Vec<i32> = Vec::with_capacity(words + 2 * HEADROOM);
    // SAFETY: the capacity holds the headroom before the buffer
    let ptr = unsafe { buffer.as_mut_ptr().add(HEADROOM) };
    ALLOCATIONS.lock().unwrap().push(Allocation { buffer, input: Some(words) });
    ptr
}

/// Frees every allocation and result since the last reset.
pub fn reset() {
    ALLOCATIONS.lock().unwrap().clear();
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
    let allocation = allocations.iter_mut().rev().filter_map(|a| a.input.map(|words| (words, a))).find_map(|(words, a)| {
        // SAFETY: inside the capacity, see `alloc`
        let start = unsafe { a.buffer.as_mut_ptr().add(HEADROOM) };
        (start as *const i32 == input.as_ptr() && words == input.len()).then_some(a)
    })?;
    allocation.input = Some(0);
    let base = allocation.buffer.as_mut_ptr();
    let first = HEADROOM - prefix.len();
    // SAFETY: the prefix ends where the input starts and the suffix starts where it ends, both in the
    // headroom of the capacity, which no reference to the input covers
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
    let mut buffer = vec![0; words];
    let len = write(&mut buffer);
    assert!(len <= words, "a result of {len} words in a buffer of {words}");
    let ptr = buffer.as_ptr();
    ALLOCATIONS.lock().unwrap().push(Allocation { buffer, input: None });
    InPlace { ptr, len }
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
                allocations.push(Allocation { buffer, input: None });
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
            let result = &allocations.last().unwrap().buffer;
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
