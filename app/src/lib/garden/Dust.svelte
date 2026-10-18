<script lang="ts">
  import { T, useTask } from '@threlte/core';
  import { AdditiveBlending, BufferAttribute, BufferGeometry, Color, ShaderMaterial } from 'three';
  import { BED_DEPTH, BED_WIDTH } from './config';
  import { createRandom } from './random';

  type Props = {
    color: string;
    count?: number;
    /** size of the specks in pixels at a distance of 12 */
    size?: number;
    opacity?: number;
    /** how far the specks spread around the bed */
    spread?: number;
  };
  const { color, count = 260, size = 3, opacity = 0.45, spread = 4 }: Props = $props();

  /** Specks of dust and pollen that slowly drift through the light */
  function createSpecks(count: number, spread: number) {
    const random = createRandom(9);
    const positions = new Float32Array(count * 3);
    const seeds = new Float32Array(count);
    for (let i = 0; i < count; i++) {
      positions[i * 3] = (random() - 0.5) * (BED_WIDTH + spread);
      positions[i * 3 + 1] = 0.15 + Math.pow(random(), 1.5) * 4.5;
      positions[i * 3 + 2] = (random() - 0.5) * (BED_DEPTH + spread);
      seeds[i] = random();
    }
    const geometry = new BufferGeometry();
    geometry.setAttribute('position', new BufferAttribute(positions, 3));
    geometry.setAttribute('seed', new BufferAttribute(seeds, 1));
    return geometry;
  }
  const geometry = $derived(createSpecks(count, spread));

  const material = new ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: AdditiveBlending,
    uniforms: {
      time: { value: 0 },
      color: { value: new Color() },
      size: { value: 1 },
      opacity: { value: 1 },
      pixelRatio: { value: window.devicePixelRatio }
    },
    vertexShader: /* glsl */ `
      uniform float time;
      uniform float pixelRatio;
      uniform float size;
      attribute float seed;
      varying float vAlpha;

      void main() {
        float t = time * (0.04 + seed * 0.05) + seed * 40.0;
        vec3 drift = vec3(
          sin(t * 1.3) * 0.6 + sin(t * 0.37 + seed * 9.0) * 0.4,
          sin(t * 0.9 + seed * 4.0) * 0.25,
          cos(t * 1.1) * 0.5
        );
        vec4 mvPosition = modelViewMatrix * vec4(position + drift, 1.0);
        gl_Position = projectionMatrix * mvPosition;
        gl_PointSize = size * (0.6 + seed) * pixelRatio * (12.0 / -mvPosition.z);
        // every speck slowly fades in and out
        vAlpha = 0.25 + 0.75 * pow(0.5 + 0.5 * sin(time * (0.2 + seed * 0.3) + seed * 30.0), 2.0);
      }
    `,
    fragmentShader: /* glsl */ `
      uniform vec3 color;
      uniform float opacity;
      varying float vAlpha;

      void main() {
        float d = length(gl_PointCoord - 0.5);
        float alpha = smoothstep(0.5, 0.0, d) * vAlpha * opacity;
        gl_FragColor = vec4(color * alpha, alpha);
      }
    `
  });

  $effect(() => {
    material.uniforms.color.value.set(color);
    material.uniforms.size.value = size;
    material.uniforms.opacity.value = opacity;
  });

  useTask((delta) => {
    material.uniforms.time.value += delta;
  });

  $effect(() => {
    const current = geometry;
    return () => current.dispose();
  });

  $effect(() => () => material.dispose());
</script>

<T.Points {geometry} {material} frustumCulled={false} />
