use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

extern "C" {
    fn __wasm_memory_size() -> usize;
    fn __nodarium_manual_end() -> usize;
}

#[allow(dead_code)]
const WASM_PAGE_SIZE: usize = 64 * 1024;

pub struct UpwardBumpAllocator {
    heap_base: AtomicUsize,
}

impl Default for UpwardBumpAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl UpwardBumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_base: AtomicUsize::new(0),
        }
    }

    #[allow(dead_code)]
    pub fn init(&self) {
        // Start heap at 10000 to leave space for data sections
        self.heap_base.store(10000, Ordering::Relaxed);
    }
}

#[global_allocator]
pub static ALLOCATOR: UpwardBumpAllocator = UpwardBumpAllocator::new();

unsafe impl GlobalAlloc for UpwardBumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();

        let mut current = self.heap_base.load(Ordering::Relaxed);

        loop {
            let aligned = (current + align - 1) & !(align - 1);
            let new_current = aligned + size;

            let manual_end = unsafe { __nodarium_manual_end() };
            if new_current > manual_end {
                return core::ptr::null_mut();
            }

            match self.heap_base.compare_exchange(
                current,
                new_current,
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
