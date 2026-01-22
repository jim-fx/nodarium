pub fn encode_float(f: f32) -> i32 {
    // Convert f32 to u32 using to_bits, then safely cast to i32
    let bits = f.to_bits();
    bits as i32
}

pub fn decode_float(bits: i32) -> f32 {
    // Convert i32 to u32 safely, then use from_bits to get f32
    let bits = bits as u32;
    f32::from_bits(bits)
}

#[inline]
pub unsafe fn read_i32(ptr: *const i32) -> i32 {
    *ptr
}

#[inline]
pub unsafe fn read_f32(ptr: *const i32) -> f32 {
    f32::from_bits(*ptr as u32)
}

#[inline]
pub unsafe fn read_bool(ptr: *const i32) -> bool {
    *ptr != 0
}

#[inline]
pub unsafe fn read_vec3(ptr: *const i32) -> [f32; 3] {
    let p = ptr as *const f32;
    [p.read(), p.add(1).read(), p.add(2).read()]
}

#[inline]
pub unsafe fn read_i32_slice(ptr: *const i32, len: usize) -> Vec<i32> {
    std::slice::from_raw_parts(ptr, len).to_vec()
}

#[inline]
pub unsafe fn read_f32_slice(ptr: *const i32, len: usize) -> Vec<f32> {
    std::slice::from_raw_parts(ptr as *const f32, len).to_vec()
}

#[inline]
pub unsafe fn read_f32_default(ptr: *const i32, default: f32) -> f32 {
    if ptr.is_null() {
        default
    } else {
        read_f32(ptr)
    }
}

#[inline]
pub unsafe fn read_i32_default(ptr: *const i32, default: i32) -> i32 {
    if ptr.is_null() {
        default
    } else {
        read_i32(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_float_simple() {
        let test_values: [f32; 6] = [
            0.0,
            -0.0,
            123.456,
            -123.456,
            std::f32::INFINITY,
            std::f32::NEG_INFINITY,
        ];
        for &value in &test_values {
            let encoded = encode_float(value);
            let decoded = decode_float(encoded);
            assert_eq!(
                decoded.to_bits(),
                value.to_bits(),
                "Failed for value {}",
                value
            );
        }
    }
}
