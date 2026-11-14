import { AmbientLight, Color, DirectionalLight, HemisphereLight, Vector3 } from 'three';
import { describe, expect, it } from 'vitest';
import { createLights, lighting, shaderLights } from './lighting';

const CAMERA_DIRECTION = new Vector3(1, 1, 1).normalize();

describe('lighting', () => {
  it('lights from above and to the side, not from the camera', () => {
    const key = createLights().find((l) => l instanceof DirectionalLight)!;
    const direction = key.position.clone().normalize();
    expect(direction.y).toBeGreaterThan(0.5);
    expect(direction.dot(CAMERA_DIRECTION)).toBeLessThan(0.8);
  });

  it('keeps the fill and the flat ambient light well below the key light', () => {
    const lights = createLights();
    const key = lights.find((l) => l instanceof DirectionalLight)!;
    const fill = lights.find((l) => l instanceof HemisphereLight)!;
    const ambient = lights.find((l) => l instanceof AmbientLight)!;
    expect(fill.intensity).toBeLessThan(key.intensity / 2);
    expect(ambient.intensity).toBeLessThan(fill.intensity / 2);
  });

  it('gives the shaders the same key light as the physically based materials', () => {
    const lights = shaderLights();
    const expected = new Color(lighting.key.color).multiplyScalar(lighting.key.intensity / Math.PI);
    expect(lights.color.r).toBeCloseTo(expected.r, 6);
    expect(lights.color.g).toBeCloseTo(expected.g, 6);
    expect(lights.color.b).toBeCloseTo(expected.b, 6);
    expect(lights.direction.equals(lighting.key.direction)).toBe(true);
    expect(lights.sky.r).toBeGreaterThan(lights.ground.r);
  });
});
