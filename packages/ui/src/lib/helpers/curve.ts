// The curve of the curve node: a monotone cubic (Steffen) through points
// [x0, y0, x1, y1, ...] sorted by x. It has to match `curve` in packages/utils/src/expr.rs.

function slope(points: number[], i: number) {
  const h = points[i * 2 + 2] - points[i * 2];
  return h > 0 ? (points[i * 2 + 3] - points[i * 2 + 1]) / h : 0;
}

export function curveTangents(points: number[]) {
  const n = points.length / 2;
  const tangents: number[] = [];
  for (let i = 0; i < n; i++) {
    if (i === 0) {
      tangents.push(slope(points, 0));
    } else if (i === n - 1) {
      tangents.push(slope(points, n - 2));
    } else {
      const s0 = slope(points, i - 1);
      const s1 = slope(points, i);
      if (s0 * s1 <= 0) {
        tangents.push(0);
        continue;
      }
      const h0 = points[i * 2] - points[i * 2 - 2];
      const h1 = points[i * 2 + 2] - points[i * 2];
      const p = (s0 * h1 + s1 * h0) / (h0 + h1);
      tangents.push(Math.sign(s0) * 2 * Math.min(Math.abs(s0), Math.abs(s1), 0.5 * Math.abs(p)));
    }
  }
  return tangents;
}

export function evaluateCurve(points: number[], x: number) {
  const n = points.length / 2;
  if (n < 2) return x;

  const tangents = curveTangents(points);
  let i = 0;
  while (i < n - 2 && x > points[i * 2 + 2]) i++;

  const h = points[i * 2 + 2] - points[i * 2];
  if (h <= 0) return points[i * 2 + 3];

  const t = Math.min(Math.max((x - points[i * 2]) / h, 0), 1);
  const t2 = t * t;
  const t3 = t2 * t;
  return (2 * t3 - 3 * t2 + 1) * points[i * 2 + 1]
    + (t3 - 2 * t2 + t) * h * tangents[i]
    + (-2 * t3 + 3 * t2) * points[i * 2 + 3]
    + (t3 - t2) * h * tangents[i + 1];
}

/** SVG path of the curve in a 100 x 100 box, y pointing up */
export function curvePath(points: number[]) {
  const n = points.length / 2;
  if (n < 2) return '';

  const tangents = curveTangents(points);
  const x = (i: number) => points[i * 2] * 100;
  const y = (i: number) => 100 - points[i * 2 + 1] * 100;

  // a cubic hermite segment is a bezier with its control points a third along the tangents
  let d = `M ${x(0)} ${y(0)}`;
  for (let i = 0; i < n - 1; i++) {
    const h = (x(i + 1) - x(i)) / 3;
    d += ` C ${x(i) + h} ${y(i) - tangents[i] * h}`
      + ` ${x(i + 1) - h} ${y(i + 1) + tangents[i + 1] * h}`
      + ` ${x(i + 1)} ${y(i + 1)}`;
  }
  return d;
}
