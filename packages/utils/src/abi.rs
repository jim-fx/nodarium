//! Runtime support for the Nodarium node ABI v1, see `docs/ABI.md`.
//!
//! Every buffer handed to the host (inputs it allocated, results we produced)
//! stays alive until the host calls `nodarium_reset`, so the host can pass a
//! result pointer straight into the next node without copying it out first.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

static ALLOCATIONS: Mutex<Vec<Vec<i32>>> = Mutex::new(Vec::new());

// Result descriptor returned by `nodarium_execute`: [ptr, len]
static RESULT: [AtomicU32; 2] = [AtomicU32::new(0), AtomicU32::new(0)];

#[cfg(target_arch = "wasm32")]
extern "C" {
    fn nodarium_panic(ptr: *const u8, len: usize);
}

/// Allocates `bytes` of 4-byte aligned memory that stays valid until `reset`.
pub fn alloc(bytes: usize) -> *mut i32 {
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

/// Keeps `result` alive until the next reset and returns a pointer to its
/// `[ptr, len]` descriptor.
pub fn store_result(result: Vec<i32>) -> *const u32 {
    RESULT[0].store(result.as_ptr() as u32, Ordering::Relaxed);
    RESULT[1].store(result.len() as u32, Ordering::Relaxed);
    ALLOCATIONS.lock().unwrap().push(result);
    RESULT.as_ptr() as *const u32
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
}
