//! Four f32 lanes, four middles measured at once. On wasm they are a SIMD vector, elsewhere an
//! array. Both give the same results, bit for bit: the lanes don't mix and every lane does what
//! the scalar code would.

#[cfg(target_arch = "wasm32")]
mod imp {
    use core::arch::wasm32::*;

    #[derive(Clone, Copy)]
    pub struct F4(v128);

    impl F4 {
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn splat(x: f32) -> F4 {
            F4(f32x4_splat(x))
        }

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn load(x: &[f32; 4]) -> F4 {
            // SAFETY: `x` is 16 bytes, v128_load doesn't need them aligned
            F4(unsafe { v128_load(x.as_ptr().cast()) })
        }

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn store(self, x: &mut [f32; 4]) {
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

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn div(self, o: F4) -> F4 {
            F4(f32x4_div(self.0, o.0))
        }

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn sqrt(self) -> F4 {
            F4(f32x4_sqrt(self.0))
        }

        /// `if x < lo { lo } else { x }`
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn at_least(self, lo: F4) -> F4 {
            F4(f32x4_pmax(self.0, lo.0))
        }

        /// `if o < x { o } else { x }`
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn at_most(self, o: F4) -> F4 {
            F4(f32x4_pmin(self.0, o.0))
        }

        /// A bit for every lane that equals `o`
        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn equal(self, o: F4) -> u32 {
            i32x4_bitmask(f32x4_eq(self.0, o.0)) as u32
        }

        #[inline]
        #[target_feature(enable = "simd128")]
        pub fn least(self) -> f32 {
            let a = f32x4_pmin(self.0, i32x4_shuffle::<2, 3, 0, 1>(self.0, self.0));
            let b = f32x4_pmin(a, i32x4_shuffle::<1, 0, 3, 2>(a, a));
            f32x4_extract_lane::<0>(b)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
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
        pub fn splat(x: f32) -> F4 {
            F4([x; 4])
        }

        pub fn load(x: &[f32; 4]) -> F4 {
            F4(*x)
        }

        pub fn store(self, x: &mut [f32; 4]) {
            *x = self.0;
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

        pub fn div(self, o: F4) -> F4 {
            lanes!(self, o, |a, b| a / b)
        }

        pub fn sqrt(self) -> F4 {
            lanes!(self, self, |a, _b| a.sqrt())
        }

        pub fn at_least(self, lo: F4) -> F4 {
            lanes!(self, lo, |x, lo| if x < lo { lo } else { x })
        }

        pub fn at_most(self, o: F4) -> F4 {
            lanes!(self, o, |x, o| if o < x { o } else { x })
        }

        pub fn equal(self, o: F4) -> u32 {
            (0..4).fold(0, |bits, i| bits | ((self.0[i] == o.0[i]) as u32) << i)
        }

        pub fn least(self) -> f32 {
            let v = self.0;
            let a = [if v[2] < v[0] { v[2] } else { v[0] }, if v[3] < v[1] { v[3] } else { v[1] }];
            if a[1] < a[0] { a[1] } else { a[0] }
        }
    }
}

pub use imp::F4;
