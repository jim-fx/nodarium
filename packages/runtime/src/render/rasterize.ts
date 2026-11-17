import { bounds, INSTANCE_COLOR, STEM_COLOR, type Triangle, type Vec3 } from './triangles';

export const BACKGROUND = [236, 233, 224];
const GROUND = [150, 130, 100];
// rendered larger and scaled down, so thin stems don't break up into dots
const SUPERSAMPLING = 2;

/** The directions a view looks from, all drawn at the same scale so they can be compared */
export const VIEWS = {
  front: { yaw: 0, pitch: 0 },
  side: { yaw: Math.PI / 2, pitch: 0 },
  top: { yaw: 0.6, pitch: -1.15 },
  below: { yaw: 0.6, pitch: 1.15 }
} as const;

export type View = keyof typeof VIEWS;

export type Image = { pixels: Uint8Array; width: number; height: number };

/** The part of space the views show: its center and the length that fills a view */
export type Frame = { center: Vec3; extent: number };

/** The frame of everything within `min` and `max`, with the ground at 0 in it */
export function frameOf({ min, max }: { min: Vec3; max: Vec3 }): Frame {
  // the ground stays in the picture, the plant grows from the origin
  const bottom = Math.min(min[1], 0), top = Math.max(max[1], 0);
  const center: Vec3 = [(min[0] + max[0]) / 2, (bottom + top) / 2, (min[2] + max[2]) / 2];
  const extent = Math.max(max[0] - min[0], top - bottom, max[2] - min[2], 1e-6) * 1.08;
  return { center, extent };
}

/** The views of the triangles next to each other, `viewSize` pixels each, as RGB */
export function rasterize(
  triangles: Triangle[],
  viewSize: number,
  views: readonly View[],
  { center, extent }: Frame = frameOf(bounds(triangles))
): Image {
  const size = viewSize * SUPERSAMPLING;
  const width = size * views.length;
  const pixels = new Uint8Array(width * size * 3);
  for (let i = 0; i < pixels.length; i += 3) pixels.set(BACKGROUND, i);
  const depth = new Float32Array(width * size).fill(Infinity);
  const light = [0.4, 0.8, 0.45].map((v) => v / Math.hypot(0.4, 0.8, 0.45));

  views.map((name) => VIEWS[name]).forEach(({ yaw, pitch }, view) => {
    const offset = view * size;
    const cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(pitch), sp = Math.sin(pitch);
    const project = (v: Vec3): Vec3 => {
      const x = v[0] - center[0], y = v[1] - center[1], z = v[2] - center[2];
      const x1 = cy * x - sy * z, z1 = sy * x + cy * z;
      return [
        offset + size / 2 + (x1 / extent) * size,
        size / 2 - ((cp * y - sp * z1) / extent) * size,
        sp * y + cp * z1
      ];
    };

    if (pitch === 0) {
      const ground = Math.round(project([center[0], 0, center[2]])[1]);
      for (let x = offset; x < offset + size && ground >= 0 && ground < size; x++) {
        pixels.set(GROUND, (ground * width + x) * 3);
      }
    }

    for (const t of triangles) {
      const u = [t.b[0] - t.a[0], t.b[1] - t.a[1], t.b[2] - t.a[2]];
      const v = [t.c[0] - t.a[0], t.c[1] - t.a[1], t.c[2] - t.a[2]];
      const n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
      const length = Math.hypot(n[0], n[1], n[2]) || 1;
      // both sides of a leaf are lit the same
      const shade = 0.35
        + 0.65 * Math.abs((n[0] * light[0] + n[1] * light[1] + n[2] * light[2]) / length);
      const color = (t.color ?? (t.instanced ? INSTANCE_COLOR : STEM_COLOR)).map((c) =>
        Math.min(255, c * shade)
      );

      const [p0, p1, p2] = [project(t.a), project(t.b), project(t.c)];
      const area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
      if (Math.abs(area) < 1e-9) continue;
      const x0 = Math.max(offset, Math.floor(Math.min(p0[0], p1[0], p2[0])));
      const x1 = Math.min(offset + size - 1, Math.ceil(Math.max(p0[0], p1[0], p2[0])));
      const y0 = Math.max(0, Math.floor(Math.min(p0[1], p1[1], p2[1])));
      const y1 = Math.min(size - 1, Math.ceil(Math.max(p0[1], p1[1], p2[1])));

      for (let y = y0; y <= y1; y++) {
        for (let x = x0; x <= x1; x++) {
          const px = x + 0.5, py = y + 0.5;
          const w0 = ((p1[0] - px) * (p2[1] - py) - (p2[0] - px) * (p1[1] - py)) / area;
          const w1 = ((p2[0] - px) * (p0[1] - py) - (p0[0] - px) * (p2[1] - py)) / area;
          const w2 = 1 - w0 - w1;
          if (w0 < 0 || w1 < 0 || w2 < 0) continue;
          const z = w0 * p0[2] + w1 * p1[2] + w2 * p2[2];
          const k = y * width + x;
          if (z >= depth[k]) continue;
          depth[k] = z;
          pixels.set(color, k * 3);
        }
      }
    }
  });

  return downsample(pixels, width, size);
}

function downsample(pixels: Uint8Array, width: number, height: number): Image {
  const w = width / SUPERSAMPLING, h = height / SUPERSAMPLING;
  const out = new Uint8Array(w * h * 3);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      for (let c = 0; c < 3; c++) {
        let sum = 0;
        for (let dy = 0; dy < SUPERSAMPLING; dy++) {
          for (let dx = 0; dx < SUPERSAMPLING; dx++) {
            sum += pixels[((y * SUPERSAMPLING + dy) * width + x * SUPERSAMPLING + dx) * 3 + c];
          }
        }
        out[(y * w + x) * 3 + c] = sum / (SUPERSAMPLING * SUPERSAMPLING);
      }
    }
  }
  return { pixels: out, width: w, height: h };
}
