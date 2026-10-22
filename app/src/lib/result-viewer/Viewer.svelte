<script lang="ts">
  import SmallPerformanceViewer from '#lib/performance/SmallPerformanceViewer.svelte';
  import { appSettings } from '#lib/settings/app-settings.svelte.ts';
  import { splitNestedArray } from '@nodarium/utils';
  import type { PerformanceStore } from '@nodarium/utils';
  import { Canvas } from '@threlte/core';
  import { untrack } from 'svelte';
  import { Box3, type Group, type Material } from 'three';
  import { createGeometryPool, createInstancedGeometryPool } from './geometryPool';
  import type { Gizmo, GizmoChange, GizmoMode } from './gizmos';
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
    gizmos?: Gizmo[];
    ongizmochange?: (nodeId: number, change: GizmoChange) => void;
    ongizmostart?: () => void;
    ongizmoend?: () => void;
  };

  let {
    scene = $bindable(),
    centerCamera,
    debugData,
    perf,
    gizmos = [],
    ongizmochange,
    ongizmostart,
    ongizmoend
  }: Props = $props();

  let gizmoMode = $state<GizmoMode>('translate');
  const gizmoModes: { mode: GizmoMode; label: string; icon: string }[] = [
    { mode: 'translate', label: 'Move', icon: 'i-[tabler--arrows-move]' },
    { mode: 'scale', label: 'Scale', icon: 'i-[tabler--arrows-maximize]' }
  ];

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

<div class="relative" style="height: 100%">
  <Canvas>
    <Scene
      bind:this={sceneComponent}
      {centerCamera}
      {debugData}
      {gizmos}
      {gizmoMode}
      {ongizmochange}
      {ongizmostart}
      {ongizmoend}
      bind:scene
      bind:fps
    />
  </Canvas>
  {#if gizmos.some((g) => g.kind === 'force')}
    <div class="absolute top-3 left-3 flex overflow-hidden rounded border border-outline bg-layer-1">
      {#each gizmoModes as { mode, label, icon } (mode)}
        <button
          type="button"
          aria-label={label}
          aria-pressed={gizmoMode === mode}
          title={label}
          class="flex h-8 w-8 cursor-pointer items-center justify-center border-none text-text {gizmoMode === mode ? 'bg-selected text-white' : 'bg-transparent hover:bg-layer-2'}"
          onclick={() => (gizmoMode = mode)}
        >
          <span class="block h-4 w-4 {icon}"></span>
        </button>
      {/each}
    </div>
  {/if}
</div>
