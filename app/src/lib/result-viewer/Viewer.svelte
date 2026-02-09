<script lang="ts">
  import SmallPerformanceViewer from '$lib/performance/SmallPerformanceViewer.svelte';
  import { appSettings } from '$lib/settings/app-settings.svelte';
  import { decodeFloat, splitNestedArray } from '@nodarium/utils';
  import type { PerformanceStore } from '@nodarium/utils';
  import { Canvas } from '@threlte/core';
  import { DoubleSide, Vector3 } from 'three';
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
  };

  let { scene = $bindable(), centerCamera, perf }: Props = $props();

  let lines = $state<Vector3[][]>([]);

  function createLineGeometryFromEncodedData(encodedData: Int32Array) {
    const positions: Vector3[] = [];

    const amount = (encodedData.length - 1) / 4;

    for (let i = 0; i < amount; i++) {
      const x = decodeFloat(encodedData[2 + i * 4 + 0]);
      const y = decodeFloat(encodedData[2 + i * 4 + 1]);
      const z = decodeFloat(encodedData[2 + i * 4 + 2]);
      positions.push(new Vector3(x, y, z));
    }

    return positions;
  }

  export const update = function update(result: Int32Array) {
    perf.addPoint('split-result');
    const inputs = splitNestedArray(result);
    perf.endPoint();

    if (appSettings.value.debug.showStemLines) {
      perf.addPoint('create-lines');
      lines = inputs
        .map((input) => {
          if (input[0] === 0) {
            return createLineGeometryFromEncodedData(input);
          }
        })
        .filter(Boolean) as Vector3[][];
      perf.endPoint();
    }

    perf.addPoint('update-geometries');

    const { totalVertices, totalFaces } = updateGeometries(inputs, scene);
    perf.endPoint();

    perf.addPoint('total-vertices', totalVertices);
    perf.addPoint('total-faces', totalFaces);
    sceneComponent?.invalidate();
  };
</script>

{#if appSettings.value.debug.showPerformancePanel}
  <SmallPerformanceViewer {fps} store={perf} />
{/if}

<div style="height: 100%">
  <Canvas>
    <Scene
      bind:this={sceneComponent}
      {lines}
      {centerCamera}
      bind:scene
      bind:fps
    />
  </Canvas>
</div>
