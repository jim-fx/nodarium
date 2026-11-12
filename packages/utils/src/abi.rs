//! Runtime support for the Nodarium node ABI v1, see `docs/ABI.md`.
//!
//! Every buffer handed to the host (inputs it allocated, results we produced)
//! stays alive until the host calls `nodarium_reset`, so the host can pass a
//! result pointer straight into the next node without copying it out first.

use std::sync::{Mutex, Once};

static ALLOCATIONS: Mutex<Vec<Vec<i32>>> = Mutex::new(Vec::new());

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

/// Allocates `bytes` of 4-byte aligned memory that stays valid until `reset`.
pub fn alloc(bytes: usize) -> *mut i32 {
    reserve_lists();
    let mut buf: Vec<i32> = Vec::with_capacity(bytes.div_ceil(4));
    let ptr = buf.as_mut_ptr();
    ALLOCATIONS.lock().unwrap().push(buf);
    ptr
}

/// Frees every allocation and result since the last reset.
pub fn reset() {
    ALLOCATIONS.lock().unwrap().clear();
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

/// What an execute function can return: one value per output of the node.
///
/// A single `Vec<i32>` is a node with one output. Several outputs are returned
/// as a `Vec<Vec<i32>>`, an array or a tuple, in the order of `outputs` in the
/// node definition.
pub trait NodeOutputs {
    fn into_outputs(self) -> Vec<Vec<i32>>;
}

impl NodeOutputs for Vec<i32> {
    fn into_outputs(self) -> Vec<Vec<i32>> {
        vec![self]
    }
}

impl NodeOutputs for Vec<Vec<i32>> {
    fn into_outputs(self) -> Vec<Vec<i32>> {
        self
    }
}

impl<const N: usize> NodeOutputs for [Vec<i32>; N] {
    fn into_outputs(self) -> Vec<Vec<i32>> {
        self.into()
    }
}

macro_rules! impl_node_outputs_for_tuple {
    ($($name:ident),+) => {
        impl NodeOutputs for ($(impl_node_outputs_for_tuple!(@vec $name),)+) {
            #[allow(non_snake_case)]
            fn into_outputs(self) -> Vec<Vec<i32>> {
                let ($($name,)+) = self;
                vec![$($name),+]
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
/// `[ptr, len]` pairs describing them, one pair per output.
pub fn store_result(result: impl NodeOutputs) -> *const u32 {
    reserve_lists();
    let outputs = result.into_outputs();
    let mut descriptor = RESULT.lock().unwrap();
    descriptor.clear();
    let mut allocations = ALLOCATIONS.lock().unwrap();
    for output in outputs {
        descriptor.push(output.as_ptr() as u32);
        descriptor.push(output.len() as u32);
        allocations.push(output);
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
        assert_eq!(vec![1, 2].into_outputs(), vec![vec![1, 2]]);
        assert_eq!((vec![1], vec![2, 3]).into_outputs(), vec![vec![1], vec![2, 3]]);
        assert_eq!([vec![1], vec![2]].into_outputs(), vec![vec![1], vec![2]]);
        assert_eq!(vec![vec![1], vec![], vec![3]].into_outputs().len(), 3);
    }

    #[test]
    fn test_store_result_writes_one_pair_per_output() {
        let a = vec![1, 2, 3];
        let b = vec![4];
        let (a_ptr, b_ptr) = (a.as_ptr() as u32, b.as_ptr() as u32);
        let descriptor = store_result((a, b));
        let pairs = unsafe { std::slice::from_raw_parts(descriptor, 4) };
        assert_eq!(pairs, &[a_ptr, 3, b_ptr, 1]);
        reset();
    }
}
