<script lang="ts">
  import SmallPerformanceViewer from '$lib/performance/SmallPerformanceViewer.svelte';
  import { appSettings } from '$lib/settings/app-settings.svelte';
  import { splitNestedArray } from '@nodarium/utils';
  import type { PerformanceStore } from '@nodarium/utils';
  import { Canvas } from '@threlte/core';
  import { DoubleSide } from 'three';
  import { type Group, MeshMatcapMaterial, TextureLoader } from 'three';
  import { createGeometryPool, createInstancedGeometryPool } from './geometryPool';
  import Scene from './Scene.svelte';

  const loader = new TextureLoader();
  const matcap = loader.load('/matcap_green.jpg');
  matcap.colorSpace = 'srgb';
  const material = new MeshMatcapMaterial({
    color: 0xffffff,
    matcap,
    side: DoubleSide
  });

  let sceneComponent = $state<ReturnType<typeof Scene>>();
  let fps = $state<number[]>([]);

  let geometryPool: ReturnType<typeof createGeometryPool>;
  let instancePool: ReturnType<typeof createInstancedGeometryPool>;

  export function invalidate() {
    sceneComponent?.invalidate();
  }

  export function updateGeometries(inputs: Int32Array[], group: Group) {
    geometryPool = geometryPool || createGeometryPool(group, material);
    instancePool = instancePool || createInstancedGeometryPool(group, material);

    let meshes = geometryPool.update(inputs.filter((i) => i[0] === 1));
    let faces = instancePool.update(inputs.filter((i) => i[0] === 2));

    return {
      totalFaces: meshes.totalFaces + faces.totalFaces,
      totalVertices: meshes.totalVertices + faces.totalVertices
    };
  }

  type Props = {
    scene: Group;
    centerCamera: boolean;
    perf: PerformanceStore;
    debugData?: Record<number, { type: string; data: Int32Array }>;
  };

  let { scene = $bindable(), centerCamera, debugData, perf }: Props = $props();

  export const update = function update(result: Int32Array) {
    perf.addPoint('split-result');
    const inputs = splitNestedArray(result);
    perf.endPoint();

    perf.addPoint('update-geometries');

    const { totalVertices, totalFaces } = updateGeometries(inputs, scene);
    perf.endPoint();

    perf.addPoint('total-vertices', totalVertices);
    perf.addPoint('total-faces', totalFaces);
    sceneComponent?.invalidate();
  };
</script>

{#if appSettings.value.debug.advancedMode}
  <SmallPerformanceViewer {fps} store={perf} />
{/if}

<div style="height: 100%">
  <Canvas>
    <Scene
      bind:this={sceneComponent}
      {centerCamera}
      {debugData}
      bind:scene
      bind:fps
    />
  </Canvas>
</div>
