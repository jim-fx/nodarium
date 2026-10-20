<script lang="ts">
  import SmallPerformanceViewer from '$lib/performance/SmallPerformanceViewer.svelte';
  import { appSettings } from '$lib/settings/app-settings.svelte';
  import { splitNestedArray } from '@nodarium/utils';
  import type { PerformanceStore } from '@nodarium/utils';
  import { Canvas } from '@threlte/core';
  import { untrack } from 'svelte';
  import { Box3, type Group, type Material } from 'three';
  import { createGeometryPool, createInstancedGeometryPool } from './geometryPool';
  import { getMaterial, matcapLoaded, setHeightRange } from './materials';
  import Scene from './Scene.svelte';

  // created on first use, switching back and forth keeps the compiled shaders
  const materialCache: Record<string, Material> = {};
  function loadMaterial(id: string) {
    const entry = getMaterial(id);
    materialCache[entry.id] ??= entry.create();
    return materialCache[entry.id];
  }

  let material = loadMaterial(appSettings.value.material.id);
  const bounds = new Box3();

  $effect(() => {
    const id = appSettings.value.material.id;
    untrack(() => {
      material = loadMaterial(id);
      setHeightRange(material, bounds);
      geometryPool?.setMaterial(material);
      instancePool?.setMaterial(material);
      sceneComponent?.invalidate();
    });
  });

  matcapLoaded.then(() => sceneComponent?.invalidate());

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

    // the pools keep hidden meshes with stale geometry around
    bounds.makeEmpty();
    group.traverseVisible((child) => {
      if ('isMesh' in child) bounds.expandByObject(child);
    });
    setHeightRange(material, bounds);

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
