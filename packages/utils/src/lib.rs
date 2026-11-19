pub mod abi;
mod box_grid;
mod color;
mod encoding;
mod expr;
mod force;
mod inputs;
mod pattern;
mod points;
mod tree;
pub use box_grid::BoxGrid;
pub use color::*;
pub use encoding::*;
pub use expr::*;
pub use force::*;
pub use inputs::*;
pub use pattern::*;
pub use points::*;
pub use tree::*;
pub mod geometry;

extern "C" {
    #[cfg(target_arch = "wasm32")]
    pub fn nodarium_log(ptr: *const u8, len: usize);
    #[cfg(target_arch = "wasm32")]
    fn nodarium_probe(tag: i32, value: f32, path: i32);
}

/// Hands a value a probe expression gave to the host, see `nodarium probe`
pub fn probe(tag: i32, value: f32, path: i32) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        nodarium_probe(tag, value, path);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (tag, value, path);
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

