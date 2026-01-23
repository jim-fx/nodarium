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
pub fn read_i32(ptr: i32) -> i32 {
    unsafe {
        let _ptr = ptr as *const i32;
        *_ptr
    }
}

#[inline]
pub fn read_f32(ptr: i32) -> f32 {
    unsafe {
        let _ptr = ptr as *const i32;
        f32::from_bits(*_ptr as u32)
    }
}

#[inline]
pub fn read_i32_slice(range: (i32, i32)) -> Vec<i32> {
    let (start, end) = range;
    assert!(end >= start);
    let byte_len = (end - start) as usize;
    assert!(byte_len % 4 == 0);

    unsafe {
        let ptr = start as *const i32;
        let len = byte_len / 4;
        std::slice::from_raw_parts(ptr, len).to_vec()
    }
}

#[inline]
pub fn read_f32_slice(range: (i32, i32)) -> Vec<f32> {
    let (start, end) = range;
    assert!(end >= start);
    let byte_len = (end - start) as usize;
    assert!(byte_len % 4 == 0);

    unsafe {
        let ptr = start as *const f32;
        let len = byte_len / 4;
        std::slice::from_raw_parts(ptr, len).to_vec()
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
