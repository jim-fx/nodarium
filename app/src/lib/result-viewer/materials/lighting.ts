import {
  AmbientLight,
  Color,
  DirectionalLight,
  HemisphereLight,
  type Object3D,
  Vector3
} from 'three';

/**
 * The lights of the viewer and of the material previews. The key light comes from above and to the
 * left of the default camera, a weak sky/ground fill keeps the shadow side readable and the flat
 * ambient light is only there so nothing turns fully black.
 */
export const lighting = {
  key: { color: '#fff1dc', intensity: 2.2, direction: new Vector3(-0.7, 1, 0.6).normalize() },
  fill: { sky: '#cddcf5', ground: '#3b3328', intensity: 0.45 },
  ambient: { intensity: 0.05 },
  /** the plant material is lit by the meadow as well, it brings its own fill */
  environment: { url: '/environments/meadow_256.exr', intensity: 0.5 }
};

export function createLights(): Object3D[] {
  const key = new DirectionalLight(lighting.key.color, lighting.key.intensity);
  key.position.copy(lighting.key.direction);
  return [
    key,
    new HemisphereLight(lighting.fill.sky, lighting.fill.ground, lighting.fill.intensity),
    new AmbientLight(0xffffff, lighting.ambient.intensity)
  ];
}

/**
 * The same lights for a shader without the lights of three. Physically based materials divide the
 * light by pi, the shaders take it as it is.
 */
export function shaderLights() {
  const ambient = lighting.ambient.intensity / Math.PI;
  const fill = lighting.fill.intensity / Math.PI;
  const addAmbient = (c: Color) => c.addScalar(ambient);
  return {
    direction: lighting.key.direction.clone(),
    color: new Color(lighting.key.color).multiplyScalar(lighting.key.intensity / Math.PI),
    sky: addAmbient(new Color(lighting.fill.sky).multiplyScalar(fill)),
    ground: addAmbient(new Color(lighting.fill.ground).multiplyScalar(fill))
  };
}
