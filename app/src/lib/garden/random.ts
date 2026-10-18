/** Small seeded pseudo random number generator (mulberry32) */
export function createRandom(seed: number) {
  let state = Math.imul(seed ^ 0x9e3779b9, 0x85ebca6b) >>> 0;
  return function random() {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function hash3(x: number, y: number, z: number, seed: number) {
  let h = Math.imul(x, 0x27d4eb2d) ^ Math.imul(y, 0x165667b1) ^ Math.imul(z, 0x9e3779b1) ^ seed;
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d);
  h = Math.imul(h ^ (h >>> 12), 0x297a2d39);
  return ((h ^ (h >>> 15)) >>> 0) / 4294967296;
}

function smooth(t: number) {
  return t * t * (3 - 2 * t);
}

/** Smooth value noise in the range 0..1 */
export function valueNoise3(x: number, y: number, z: number, seed = 0) {
  const ix = Math.floor(x), iy = Math.floor(y), iz = Math.floor(z);
  const fx = smooth(x - ix), fy = smooth(y - iy), fz = smooth(z - iz);
  const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
  const corner = (dx: number, dy: number, dz: number) => hash3(ix + dx, iy + dy, iz + dz, seed);
  return lerp(
    lerp(
      lerp(corner(0, 0, 0), corner(1, 0, 0), fx),
      lerp(corner(0, 1, 0), corner(1, 1, 0), fx),
      fy
    ),
    lerp(
      lerp(corner(0, 0, 1), corner(1, 0, 1), fx),
      lerp(corner(0, 1, 1), corner(1, 1, 1), fx),
      fy
    ),
    fz
  );
}

/** Layered value noise in the range 0..1 */
export function fbm3(x: number, y: number, z: number, seed = 0, octaves = 4) {
  let sum = 0, amplitude = 0.5, total = 0;
  for (let i = 0; i < octaves; i++) {
    sum += valueNoise3(x, y, z, seed + i * 1013) * amplitude;
    total += amplitude;
    x *= 2;
    y *= 2;
    z *= 2;
    amplitude *= 0.5;
  }
  return sum / total;
}
