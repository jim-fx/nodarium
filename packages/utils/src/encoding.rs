pub fn encode_float(f: f32) -> i32 {
    let bits = f.to_bits();
    bits as i32
}

pub fn decode_float(bits: i32) -> f32 {
    let bits = bits as u32;
    f32::from_bits(bits)
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
