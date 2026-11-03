import { Euler, Quaternion, Vector3 } from 'three';
import { describe, expect, it } from 'vitest';
import { flipRotation, NO_FLIP, pickFlip } from './readable-text';

const origin = new Vector3();
const worldUp = new Vector3(0, 1, 0);
const upright = new Quaternion();
// lying on the floor, readable from above with its top pointing to -z
const flat = new Quaternion().setFromEuler(new Euler(-Math.PI / 2, 0, 0));

/** Where the text's axes end up in world space once the flip is applied */
function axes(orientation: Quaternion, flip: ReturnType<typeof pickFlip>) {
  const q = orientation.clone().multiply(
    new Quaternion().setFromEuler(new Euler(...flipRotation(flip)))
  );
  return {
    right: new Vector3(1, 0, 0).applyQuaternion(q),
    up: new Vector3(0, 1, 0).applyQuaternion(q),
    normal: new Vector3(0, 0, 1).applyQuaternion(q)
  };
}

describe('pickFlip', () => {
  it('keeps the text as is when seen from the front', () => {
    expect(pickFlip(upright, origin, new Vector3(0, 0, 5), worldUp)).toBe(NO_FLIP);
  });

  it('turns around when seen from behind', () => {
    const camera = new Vector3(0, 0, -5);
    const flip = pickFlip(upright, origin, camera, worldUp);
    expect(flip).toEqual({ right: -1, up: 1 });
    expect(axes(upright, flip).normal.dot(camera)).toBeGreaterThan(0);
    expect(axes(upright, flip).up.dot(worldUp)).toBeCloseTo(1);
  });

  it('turns upside down when the camera rolls over', () => {
    const flip = pickFlip(upright, origin, new Vector3(0, 0, 5), new Vector3(0, -1, 0));
    expect(flip).toEqual({ right: -1, up: -1 });
  });

  it('turns flat text towards a camera on the other side of it', () => {
    // camera behind the text's top edge, looking down and towards +z, so its screen up is -z
    const camera = new Vector3(0, 5, -5);
    const cameraUp = new Vector3(0, 1, 1).normalize();
    const flip = pickFlip(flat, origin, camera, cameraUp);
    const { normal, up } = axes(flat, flip);
    expect(normal.dot(camera)).toBeGreaterThan(0);
    expect(up.dot(cameraUp)).toBeGreaterThan(0);
  });

  it('flips flat text when the camera goes below the floor', () => {
    const camera = new Vector3(0, -5, 5);
    const cameraUp = new Vector3(0, 1, 1).normalize();
    const flip = pickFlip(flat, origin, camera, cameraUp);
    const { normal, up } = axes(flat, flip);
    expect(normal.dot(camera)).toBeGreaterThan(0);
    expect(up.dot(cameraUp)).toBeGreaterThan(0);
  });

  it('never mirrors the text', () => {
    for (const right of [1, -1] as const) {
      for (const up of [1, -1] as const) {
        const a = axes(upright, { right, up });
        expect(a.right.clone().cross(a.up).dot(a.normal)).toBeCloseTo(1);
      }
    }
  });

  it('does not flip back and forth when seen edge on', () => {
    const behind = pickFlip(upright, origin, new Vector3(0, 0, -5), worldUp);
    const edgeOn = new Vector3(5, 0, 0.2);
    expect(pickFlip(upright, origin, edgeOn, worldUp, behind)).toBe(behind);
    expect(pickFlip(upright, origin, edgeOn, worldUp, NO_FLIP)).toBe(NO_FLIP);
  });
});
