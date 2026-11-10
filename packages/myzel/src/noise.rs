use glam::Vec3;

// integer hash so the same lattice values can be reproduced in a shader
fn hash(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f)
        ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
}

fn fade(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Smoothly interpolated lattice noise in [-1, 1]
pub fn value_noise(p: Vec3, seed: u32) -> f32 {
    let cell = p.floor();
    let (x, y, z) = (cell.x as i32, cell.y as i32, cell.z as i32);
    let f = p - cell;
    let (u, v, w) = (fade(f.x), fade(f.y), fade(f.z));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let plane = |z: i32| {
        lerp(
            lerp(hash(x, y, z, seed), hash(x + 1, y, z, seed), u),
            lerp(hash(x, y + 1, z, seed), hash(x + 1, y + 1, z, seed), u),
            v,
        )
    };
    lerp(plane(z), plane(z + 1), w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_in_range_and_is_deterministic() {
        for i in 0..500 {
            let p = Vec3::new(i as f32 * 0.37, i as f32 * -0.21, i as f32 * 0.13);
            let n = value_noise(p, 3);
            assert!((-1.0..=1.0).contains(&n));
            assert_eq!(n, value_noise(p, 3));
        }
    }

    #[test]
    fn seed_changes_the_noise() {
        let p = Vec3::new(0.5, 1.5, 2.5);
        assert_ne!(value_noise(p, 1), value_noise(p, 2));
    }
}
