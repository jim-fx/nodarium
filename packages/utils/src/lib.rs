pub mod abi;
mod box_grid;
mod encoding;
mod expr;
mod force;
mod pattern;
mod points;
mod tree;
pub use box_grid::BoxGrid;
pub use encoding::*;
pub use expr::*;
pub use force::*;
pub use pattern::*;
pub use points::*;
pub use tree::*;
pub mod geometry;

extern "C" {
    #[cfg(target_arch = "wasm32")]
    pub fn nodarium_log(ptr: *const u8, len: usize);
}

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! log {
    ($($t:tt)*) => {{
        let msg = std::format!($($t)*);
        #[cfg(target_arch = "wasm32")]
        unsafe {
            $crate::nodarium_log(msg.as_ptr(), msg.len());
        }
        #[cfg(not(target_arch = "wasm32"))]
        println!("{}", msg);
    }}
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        // This will expand to nothing in release builds
    }};
}

