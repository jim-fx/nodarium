import { type EulerTuple, type Quaternion, Vector3 } from 'three';

/**
 * Signs for the text's right and up axes. The facing axis is their product,
 * so every combination is a proper rotation and the text is never mirrored.
 */
export type Flip = { right: 1 | -1; up: 1 | -1 };

export const NO_FLIP: Flip = { right: 1, up: 1 };

// a sign only changes once the camera is clearly on the other side, so text seen edge-on doesn't flicker
const HYSTERESIS = 0.1;

const textUp = new Vector3();
const textNormal = new Vector3();
const toCamera = new Vector3();

/**
 * Picks the orientation of the text that reads correctly from the camera.
 * `orientation` is the text's own world rotation, before any flip.
 */
export function pickFlip(
  orientation: Quaternion,
  position: Vector3,
  cameraPosition: Vector3,
  cameraUp: Vector3,
  previous: Flip = NO_FLIP
): Flip {
  textUp.set(0, 1, 0).applyQuaternion(orientation);
  textNormal.set(0, 0, 1).applyQuaternion(orientation);
  toCamera.subVectors(cameraPosition, position).normalize();

  let up = previous.up;
  if (up * textUp.dot(cameraUp) < -HYSTERESIS) up = -up as 1 | -1;

  let facing = (previous.right * previous.up) as 1 | -1;
  if (facing * textNormal.dot(toCamera) < -HYSTERESIS) facing = -facing as 1 | -1;

  const right = (facing * up) as 1 | -1;
  if (right === previous.right && up === previous.up) return previous;
  return { right, up };
}

/** Local rotation that applies a flip, a half turn around the one axis that keeps its sign */
export function flipRotation({ right, up }: Flip): EulerTuple {
  if (right === 1 && up === 1) return [0, 0, 0];
  if (right === -1 && up === -1) return [0, 0, Math.PI];
  if (right === -1) return [0, Math.PI, 0];
  return [Math.PI, 0, 0];
}
