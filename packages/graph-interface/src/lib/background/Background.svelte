<script lang="ts">
  import { T, useThrelte } from '@threlte/core';
  import { getGraphState } from '../graph-state.svelte';
  import BackgroundFrag from './Background.frag?raw';
  import BackgroundVert from './Background.vert?raw';

  const { invalidate } = useThrelte();
  const graphState = getGraphState();
  const colors = graphState.options.colors;

  type Props = {
    minZoom?: number;
    maxZoom?: number;
    cameraPosition?: [number, number, number];
    width?: number;
    height?: number;
    type?: 'grid' | 'dots' | 'none';
  };

  let {
    minZoom = 4,
    maxZoom = 150,
    cameraPosition = [0, 1, 0],
    width = globalThis?.innerWidth || 100,
    height = globalThis?.innerHeight || 100,
    type = 'grid'
  }: Props = $props();

  const typeMap = new Map([
    ['grid', 0],
    ['dots', 1],
    ['none', 2]
  ]);

  const gridType = $derived(typeMap.get(type) || 0);

  let bw = $derived(width / cameraPosition[2]);
  let bh = $derived(height / cameraPosition[2]);

  $effect(() => {
    if (graphState.options.theme) {
      setTimeout(() => invalidate(), 10);
    }
  });
</script>

<T.Group
  visible={!graphState.options.theme.includes('contrast')}
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
        },
        gridType: {
          value: 0
        }
      }}
      uniforms.camPos.value={cameraPosition}
      uniforms.backgroundColor.value={graphState.options.theme
      && colors['layer-0']}
      uniforms.lineColor.value={graphState.options.theme && colors['outline']}
      uniforms.zoomLimits.value={[minZoom, maxZoom]}
      uniforms.dimensions.value={[width, height]}
      uniforms.gridType.value={gridType}
    />
  </T.Mesh>
</T.Group>
