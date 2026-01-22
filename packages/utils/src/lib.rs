mod encoding;
mod nodes;
mod tree;
pub use encoding::*;
pub use nodes::reset_call_count;
pub use tree::*;
pub mod geometry;

extern "C" {
    #[cfg(target_arch = "wasm32")]
    pub fn __nodarium_log(ptr: *const u8, len: usize);
}

// #[cfg(debug_assertions)]
#[macro_export]
macro_rules! log {
    ($($t:tt)*) => {{
        let msg = std::format!($($t)*);
        #[cfg(target_arch = "wasm32")]
        unsafe {
            $crate::__nodarium_log(msg.as_ptr(), msg.len());
        }
        #[cfg(not(target_arch = "wasm32"))]
        println!("{}", msg);
    }}
}

// #[cfg(not(debug_assertions))]
// #[macro_export]
// macro_rules! log {
//     ($($arg:tt)*) => {{
//         // This will expand to nothing in release builds
//     }};
// }

#[allow(dead_code)]
#[rustfmt::skip]
fn test_split_args(){
    let inputs = vec![
        vec![0, 1, 0, 4, 1056964608, 1065353216, 1056964608, 1, 4, 1080872141, 1054951342, 32, 1, 1 ],
        vec![0, 4, 1056964608, 1065353216, 1056964608, 1, 4],
        vec![0, 1, 0, 3, 0, 0, 0, 5, 0, 0, 1073741824, 1073741824, 1, 1, 1, 1, 1, 4, 1065353216, 1054615798, 5, 1, 1 ],
        vec![ 0, 1, 0, 3, 0, 0, 0, 1, 4, 1073741824, 1073741824, 32, 1, 1 ],
        vec![0, 1, 0, 1, 0, 14, 0, 1056964608, 1056964608, 1056964608, 1058810102, 1056964608, 1069547520, 1056964608, 1050421494, 1056964608, 1075838976, 1056964608, 0, 1, 1, 1, 2, 13, 1, 1],
        vec![ 0, 1, 0, 2, 0, 0, 5, 0, 0, 1073741824, 1073741824, 1, 2, 0, 1, 4, 1088212173, 1083388723, 20, 1, 1 ]
    ];

    for input in inputs {
        println!("RESULT: {:?}", split_args(&input));
    }
}
