import { fbm3 } from '../random';
import { POND } from './config';

/** Points along the slightly irregular shore of the pond, counter clockwise when seen from above */
export function pondOutline(segments = 96, inset = 0) {
  const points: { x: number; z: number }[] = [];
  for (let i = 0; i < segments; i++) {
    const angle = (i / segments) * Math.PI * 2;
    const wobble = fbm3(Math.cos(angle) * 1.4, Math.sin(angle) * 1.4, 0, 7, 3) - 0.5;
    const radius = POND.radius * (1 + wobble * 0.45) - inset;
    points.push({
      x: POND.x + Math.cos(angle) * radius * 1.12,
      z: POND.z - Math.sin(angle) * radius * 0.88
    });
  }
  return points;
}
