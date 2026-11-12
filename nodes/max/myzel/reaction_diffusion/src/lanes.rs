//! Four f32 lanes, u and v of two neighbouring cells. On wasm they are a SIMD vector, elsewhere
//! an array. Both give the same results, bit for bit: the lanes don't mix and every lane does
//! what the scalar code would.

/// u and v of a cell
pub type Cell = [f32; 2];

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::Cell;
    use core::arch::wasm32::*;

    #[derive(Clone, Copy)]
    pub struct F4(v128);

    impl F4 {
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn pair(a: Cell, b: Cell) -> F4 {
            F4(f32x4(a[0], a[1], b[0], b[1]))
        }

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn load(x: &[Cell; 2]) -> F4 {
            // SAFETY: `x` is 16 bytes, v128_load doesn't need them aligned
            F4(unsafe { v128_load(x.as_ptr().cast()) })
        }

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn store(self, x: &mut [Cell; 2]) {
            // SAFETY: as in load
            unsafe { v128_store(x.as_mut_ptr().cast(), self.0) }
        }

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn add(self, o: F4) -> F4 {
            F4(f32x4_add(self.0, o.0))
        }

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn sub(self, o: F4) -> F4 {
            F4(f32x4_sub(self.0, o.0))
        }

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn mul(self, o: F4) -> F4 {
            F4(f32x4_mul(self.0, o.0))
        }

        /// `if x < lo { lo } else { x }`
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn at_least(self, lo: F4) -> F4 {
            F4(f32x4_pmax(self.0, lo.0))
        }

        /// `if x > hi { hi } else { x }`
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn at_most(self, hi: F4) -> F4 {
            F4(f32x4_pmin(self.0, hi.0))
        }

        /// `if x > 0 { x } else { 0 }`
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn positive(self) -> F4 {
            F4(f32x4_pmax(f32x4_splat(0.0), self.0))
        }

        /// The u of both cells, twice
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn uu(self) -> F4 {
            F4(i32x4_shuffle::<0, 0, 2, 2>(self.0, self.0))
        }

        /// The v of both cells, twice
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn vv(self) -> F4 {
            F4(i32x4_shuffle::<1, 1, 3, 3>(self.0, self.0))
        }

        /// The first cells of `a` and `b`
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn firsts(a: F4, b: F4) -> F4 {
            F4(i32x4_shuffle::<0, 1, 4, 5>(a.0, b.0))
        }

        /// The second cells of `a` and `b`
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn seconds(a: F4, b: F4) -> F4 {
            F4(i32x4_shuffle::<2, 3, 6, 7>(a.0, b.0))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::Cell;

    #[derive(Clone, Copy)]
    pub struct F4([f32; 4]);

    /// `$e` on every lane of `$a` and `$b`, spelled out so debug builds stay fast
    macro_rules! lanes {
        ($a:expr, $b:expr, |$x:ident, $y:ident| $e:expr) => {{
            let (a, b) = ($a.0, $b.0);
            F4([
                {
                    let ($x, $y) = (a[0], b[0]);
                    $e
                },
                {
                    let ($x, $y) = (a[1], b[1]);
                    $e
                },
                {
                    let ($x, $y) = (a[2], b[2]);
                    $e
                },
                {
                    let ($x, $y) = (a[3], b[3]);
                    $e
                },
            ])
        }};
    }

    impl F4 {

        pub fn pair(a: Cell, b: Cell) -> F4 {
            F4([a[0], a[1], b[0], b[1]])
        }

        pub fn load(x: &[Cell; 2]) -> F4 {
            F4::pair(x[0], x[1])
        }

        pub fn store(self, x: &mut [Cell; 2]) {
            *x = [[self.0[0], self.0[1]], [self.0[2], self.0[3]]];
        }

        pub fn add(self, o: F4) -> F4 {
            lanes!(self, o, |a, b| a + b)
        }

        pub fn sub(self, o: F4) -> F4 {
            lanes!(self, o, |a, b| a - b)
        }

        pub fn mul(self, o: F4) -> F4 {
            lanes!(self, o, |a, b| a * b)
        }

        pub fn at_least(self, lo: F4) -> F4 {
            lanes!(self, lo, |x, lo| if x < lo { lo } else { x })
        }

        pub fn at_most(self, hi: F4) -> F4 {
            lanes!(self, hi, |x, hi| if x > hi { hi } else { x })
        }

        pub fn positive(self) -> F4 {
            lanes!(F4([0.0; 4]), self, |zero, x| if x > zero { x } else { zero })
        }

        pub fn uu(self) -> F4 {
            F4([self.0[0], self.0[0], self.0[2], self.0[2]])
        }

        pub fn vv(self) -> F4 {
            F4([self.0[1], self.0[1], self.0[3], self.0[3]])
        }

        pub fn firsts(a: F4, b: F4) -> F4 {
            F4([a.0[0], a.0[1], b.0[0], b.0[1]])
        }

        pub fn seconds(a: F4, b: F4) -> F4 {
            F4([a.0[2], a.0[3], b.0[2], b.0[3]])
        }
    }
}

pub use imp::F4;
