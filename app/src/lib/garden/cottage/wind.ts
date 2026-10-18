import type { Material } from 'three';

/** Shared by every swaying material, advanced once per frame */
export const windUniforms = {
  windTime: { value: 0 }
};

/**
 * Lets the vertices of an instanced, world placed mesh sway in the wind.
 * The offset grows with the height above `base`, so the roots stay in place.
 */
export function addWind(material: Material, strength: number, base: number) {
  material.onBeforeCompile = (shader) => {
    shader.uniforms.windTime = windUniforms.windTime;
    shader.vertexShader = shader.vertexShader
      .replace(
        '#include <common>',
        /* glsl */ `#include <common>
        uniform float windTime;`
      )
      .replace(
        '#include <project_vertex>',
        /* glsl */ `
        vec4 mvPosition = vec4(transformed, 1.0);
        #ifdef USE_INSTANCING
          mvPosition = instanceMatrix * mvPosition;
        #endif
        mvPosition = modelMatrix * mvPosition;
        float windHeight = max(mvPosition.y - (${base.toFixed(3)}), 0.0);
        vec2 gust = vec2(
          sin(windTime * 1.1 + mvPosition.x * 0.35 + mvPosition.z * 0.2)
            + 0.4 * sin(windTime * 2.7 + mvPosition.z * 1.3),
          0.6 * cos(windTime * 0.9 + mvPosition.z * 0.3 + mvPosition.x * 0.15)
        );
        mvPosition.xz += gust * windHeight * windHeight * ${strength.toFixed(4)};
        mvPosition = viewMatrix * mvPosition;
        gl_Position = projectionMatrix * mvPosition;
        `
      );
  };
  // materials with different wind settings must not share a program
  material.customProgramCacheKey = () => `wind-${strength}-${base}`;
}
