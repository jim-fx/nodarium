use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

extern "C" {
    fn __wasm_memory_size() -> usize;
    fn __nodarium_manual_end() -> usize;
}

#[allow(dead_code)]
const WASM_PAGE_SIZE: usize = 64 * 1024;

pub struct DownwardBumpAllocator {
    heap_top: AtomicUsize,
}

impl Default for DownwardBumpAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl DownwardBumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_top: AtomicUsize::new(0),
        }
    }

    #[allow(dead_code)]
    pub fn init(&self) {
        let pages = unsafe { __wasm_memory_size() };
        let mem_size = pages * WASM_PAGE_SIZE;
        self.heap_top.store(mem_size, Ordering::Relaxed);
    }
}

#[global_allocator]
pub static ALLOCATOR: DownwardBumpAllocator = DownwardBumpAllocator::new();

unsafe impl GlobalAlloc for DownwardBumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();

        let mut current = self.heap_top.load(Ordering::Relaxed);

        loop {
            let aligned = (current - size) & !(align - 1);

            let manual_end = unsafe { __nodarium_manual_end() };
            if aligned < manual_end {
                return core::ptr::null_mut();
            }

            match self.heap_top.compare_exchange(
                current,
                aligned,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return aligned as *mut u8,
                Err(next) => current = next,
            }
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
