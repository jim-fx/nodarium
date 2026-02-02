<script lang="ts">
  import { appSettings } from '$lib/settings/app-settings.svelte';
  import { T } from '@threlte/core';
  import { colors } from '../graph/colors.svelte';
  import BackgroundFrag from './Background.frag';
  import BackgroundVert from './Background.vert';

  type Props = {
    minZoom: number;
    maxZoom: number;
    cameraPosition: [number, number, number];
    width: number;
    height: number;
  };

  let {
    minZoom = 4,
    maxZoom = 150,
    cameraPosition = [0, 1, 0],
    width = globalThis?.innerWidth || 100,
    height = globalThis?.innerHeight || 100
  }: Props = $props();

  let bw = $derived(width / cameraPosition[2]);
  let bh = $derived(height / cameraPosition[2]);
</script>

<T.Group
  position.x={cameraPosition[0]}
  position.z={cameraPosition[1]}
  position.y={-1.0}
>
  <T.Mesh rotation.x={-Math.PI / 2} position.y={0.2} scale.x={bw} scale.y={bh}>
    <T.PlaneGeometry args={[1, 1]} />
    <T.ShaderMaterial
      transparent
      vertexShader={BackgroundVert}
      fragmentShader={BackgroundFrag}
      uniforms={{
        camPos: {
          value: [0, 1, 0]
        },
        backgroundColor: {
          value: colors['layer-0']
        },
        lineColor: {
          value: colors['outline']
        },
        zoomLimits: {
          value: [2, 50]
        },
        dimensions: {
          value: [100, 100]
        }
      }}
      uniforms.camPos.value={cameraPosition}
      uniforms.backgroundColor.value={appSettings.value.theme
      && colors['layer-0']}
      uniforms.lineColor.value={appSettings.value.theme && colors['outline']}
      uniforms.zoomLimits.value={[minZoom, maxZoom]}
      uniforms.dimensions.value={[width, height]}
    />
  </T.Mesh>
</T.Group>
