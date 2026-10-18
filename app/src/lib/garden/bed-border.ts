import { BoxGeometry, type BufferAttribute } from 'three';
import { BED_DEPTH, BED_WIDTH } from './config';

/** Box mapping that lets the wood grain run along the longest side of the beam */
function createBeamGeometry(width: number, height: number, depth: number, textureSize: number) {
  const geometry = new BoxGeometry(width, height, depth);
  const position = geometry.getAttribute('position');
  const normal = geometry.getAttribute('normal');
  const uv = geometry.getAttribute('uv') as BufferAttribute;
  const alongX = width > depth;
  for (let i = 0; i < position.count; i++) {
    const [x, y, z] = [position.getX(i), position.getY(i), position.getZ(i)];
    const along = alongX ? x : z;
    const across = Math.abs(normal.getY(i)) > 0.5 ? (alongX ? z : x) : y;
    uv.setXY(i, along / textureSize, across / textureSize);
  }
  return geometry;
}

/** The four wooden beams around the bed */
export function createBedBorder(
  { top, bottom, thickness, textureSize }: {
    top: number;
    bottom: number;
    thickness: number;
    /** world size of one repetition of the wood texture */
    textureSize: number;
  }
) {
  const y = (top + bottom) / 2;
  const height = top - bottom;
  const outerWidth = BED_WIDTH + thickness * 2;
  const beam = (width: number, depth: number, position: [number, number, number]) => ({
    geometry: createBeamGeometry(width, height, depth, textureSize),
    position
  });
  return [
    beam(outerWidth, thickness, [0, y, BED_DEPTH / 2 + thickness / 2]),
    beam(outerWidth, thickness, [0, y, -BED_DEPTH / 2 - thickness / 2]),
    beam(thickness, BED_DEPTH, [BED_WIDTH / 2 + thickness / 2, y, 0]),
    beam(thickness, BED_DEPTH, [-BED_WIDTH / 2 - thickness / 2, y, 0])
  ];
}
