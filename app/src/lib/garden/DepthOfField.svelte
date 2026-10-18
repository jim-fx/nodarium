<script lang="ts">
  import { useTask, useThrelte } from '@threlte/core';
  import { HalfFloatType, Vector2, type Vector3, WebGLRenderTarget } from 'three';
  import { BokehPass } from 'three/examples/jsm/postprocessing/BokehPass.js';
  import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js';
  import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js';
  import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js';
  import { ShaderPass } from 'three/examples/jsm/postprocessing/ShaderPass.js';
  import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js';

  type Props = {
    /** the point that stays sharp */
    focus: Vector3;
    aperture?: number;
    maxBlur?: number;
    /** strength of the glow around the brightest highlights */
    bloom?: number;
    /** how much the corners of the image darken */
    vignette?: number;
  };
  const { focus, aperture = 0.0012, maxBlur = 0.008, bloom = 0, vignette = 0 }: Props = $props();

  const { renderer, scene, camera, size, autoRender, renderStage } = useThrelte();

  const composer = new EffectComposer(
    renderer,
    new WebGLRenderTarget(1, 1, { type: HalfFloatType, samples: 4 })
  );
  const renderPass = new RenderPass(scene, camera.current);
  const bokehPass = new BokehPass(scene, camera.current, {
    focus: 10,
    aperture: 0.001,
    maxblur: 0.008
  });
  const bloomPass = new UnrealBloomPass(new Vector2(256, 256), 0, 0.5, 1.1);
  // the vignette shader of three.js fades to grey, this one only darkens
  const vignettePass = new ShaderPass({
    uniforms: { tDiffuse: { value: null }, darkness: { value: 0 } },
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      uniform sampler2D tDiffuse;
      uniform float darkness;
      varying vec2 vUv;
      void main() {
        vec4 texel = texture2D(tDiffuse, vUv);
        float edge = smoothstep(0.35, 0.95, length(vUv - 0.5) * 1.4);
        gl_FragColor = vec4(texel.rgb * (1.0 - darkness * edge), texel.a);
      }
    `
  });
  composer.addPass(renderPass);
  composer.addPass(bokehPass);
  composer.addPass(bloomPass);
  composer.addPass(vignettePass);
  composer.addPass(new OutputPass());

  const uniforms = bokehPass.uniforms as Record<string, { value: number }>;

  $effect(() => {
    composer.setPixelRatio(renderer.getPixelRatio());
    composer.setSize($size.width, $size.height);
  });

  $effect(() => {
    bloomPass.strength = bloom;
    bloomPass.enabled = bloom > 0;
    vignettePass.uniforms.darkness.value = vignette;
    vignettePass.enabled = vignette > 0;
  });

  $effect(() => {
    uniforms.aperture.value = aperture;
    uniforms.maxblur.value = maxBlur;
  });

  $effect(() => {
    const before = autoRender.current;
    autoRender.set(false);
    return () => autoRender.set(before);
  });

  useTask(
    (delta) => {
      renderPass.camera = camera.current;
      bokehPass.camera = camera.current;
      uniforms.focus.value = camera.current.position.distanceTo(focus);
      composer.render(delta);
    },
    { stage: renderStage, autoInvalidate: false }
  );

  $effect(() => () => composer.dispose());
</script>
