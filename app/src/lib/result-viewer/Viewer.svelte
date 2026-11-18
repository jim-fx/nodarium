<script lang="ts">
  import SmallPerformanceViewer from '#lib/performance/SmallPerformanceViewer.svelte';
  import { appSettings } from '#lib/settings/app-settings.svelte.ts';
  import { NO_PATH, type PathLink, splitNestedArray } from '@nodarium/utils';
  import type { PerformanceStore } from '@nodarium/utils';
  import { Canvas } from '@threlte/core';
  import { untrack } from 'svelte';
  import { Box3, type Group, type Material } from 'three';
  import { createResultPools } from './geometry/geometryPool';
  import type { Gizmo, GizmoChange, GizmoMode } from './gizmos/gizmos';
  import { getMaterial, matcapLoaded, setHeightRange } from './materials/materials';
  import { type PartHighlight, setSelectedStrength, visibleParts } from './provenance/highlight';
  import { type PartHit, readPathLinks } from './provenance/part-hit';
  import type { Pick, PickFormat } from './provenance/picker';
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
      pools?.setMaterial(material);
      sceneComponent?.invalidate();
    });
  });

  matcapLoaded.then(() => sceneComponent?.invalidate());

  let sceneComponent = $state<ReturnType<typeof Scene>>();
  let fps = $state<number[]>([]);

  let pools: ReturnType<typeof createResultPools>;

  export function invalidate() {
    sceneComponent?.invalidate();
  }

  export function updateGeometries(inputs: Int32Array[], group: Group) {
    if (!pools) {
      pools = createResultPools(group, material);
      pools.setPicking(picking);
    }

    // the pools know the bounds of what they show already
    const stats = pools.update(inputs);
    bounds.copy(stats.bounds);
    setHeightRange(material, bounds);

    return { totalFaces: stats.totalFaces, totalVertices: stats.totalVertices };
  }

  type Props = {
    scene: Group;
    centerCamera: boolean;
    /** The camera is remembered per project */
    projectId?: number;
    perf: PerformanceStore;
    debugData?: Record<number, { type: string; data: Int32Array }>;
    gizmos?: Gizmo[];
    ongizmochange?: (nodeId: number, change: GizmoChange) => void;
    ongizmostart?: () => void;
    ongizmoend?: () => void;
    /** Whether hovering and clicking the result reports the part under the pointer */
    picking?: boolean;
    pickFormat?: PickFormat;
    onparthover?: (hit: PartHit | undefined) => void;
    onpartclick?: (hit: PartHit | undefined, event: PointerEvent) => void;
    /** The parts to tint, dim or hide. Without it the result is shown as it is. */
    highlight?: PartHighlight;
    /** Only what `highlight` doesn't hide is framed by the camera */
    localView?: boolean;
  };

  let {
    scene = $bindable(),
    centerCamera,
    projectId,
    debugData,
    perf,
    gizmos = [],
    ongizmochange,
    ongizmostart,
    ongizmoend,
    picking = false,
    pickFormat = 'integer',
    onparthover,
    onpartclick,
    highlight,
    localView = false
  }: Props = $props();

  let items: Int32Array[] = [];
  let links: Map<number, PathLink> | undefined;

  /** The links of the paths in the result by their id, read when they are asked for first */
  let creators: Set<number> | undefined;

  /**
   * The creator ids of the graph nodes that made a tissue of the Myzel meshes in the result,
   * read from the meshes when they are asked for first
   */
  export function meshCreators() {
    return creators ??= pools?.meshCreators() ?? new Set();
  }

  export function pathLinks() {
    return links ??= readPathLinks(items);
  }

  /** for measuring: how often the highlight was applied to the objects and how long it took */
  const highlightStats = { applies: 0, lastApply: 0 };

  export function provenanceStats() {
    return { pick: sceneComponent?.pickStats(), highlight: highlightStats };
  }

  function toHit({ object, kind, index }: Pick): PartHit {
    if (kind === 'geometry') return { kind, pathId: object.userData.pathId ?? NO_PATH };
    if (kind === 'instances') return { kind, pathId: object.userData.pathIds?.[index] ?? NO_PATH };
    // which graph node made the tissue, for a mesh that brings the creators of its groups
    return { kind, pathId: NO_PATH, group: index, creator: object.userData.creators?.[index] };
  }

  function handlePick(pick: Pick | undefined, kind: 'hover' | 'click', event?: PointerEvent) {
    const hit = pick && toHit(pick);
    if (kind === 'hover') onparthover?.(hit);
    else if (event) onpartclick?.(hit, event);
  }

  /** The bounds of what the highlight leaves visible */
  function focusOf(shown: PartHighlight) {
    const { paths, instances, creators } = visibleParts(shown);
    return pools.boundsOf(paths, instances, creators);
  }

  let focused = false;
  function updateFocus() {
    const focus = localView && highlight && pools ? focusOf(highlight) : undefined;
    if (!focus && !focused) return;
    focused = !!focus;
    sceneComponent?.setFocus(focus);
  }

  $effect(() => {
    const on = picking;
    untrack(() => pools?.setPicking(on));
  });

  /** ms the tint of a new selection stays, and ms it takes to fade after that */
  const FADE_HOLD = 700;
  const FADE_OUT = 900;
  let fadeFrame = 0;
  let fadeStartedAt = 0;
  let shownSelection: string | undefined;

  function fadeStep() {
    fadeFrame = 0;
    const elapsed = performance.now() - fadeStartedAt;
    const strength = Math.min(Math.max(1 - (elapsed - FADE_HOLD) / FADE_OUT, 0), 1);
    setSelectedStrength(strength);
    sceneComponent?.redraw();
    if (strength > 0) fadeFrame = requestAnimationFrame(fadeStep);
  }

  /** The tint of the selection stays, or shows what was selected and then gives the colours back */
  function updateFade(next: PartHighlight | undefined, fade: boolean) {
    const changed = next?.selection !== shownSelection;
    shownSelection = next?.selection;
    if (!fade) {
      cancelAnimationFrame(fadeFrame);
      fadeFrame = 0;
      setSelectedStrength(1);
    } else if (changed && next) {
      fadeStartedAt = performance.now();
      setSelectedStrength(1);
      if (!fadeFrame) fadeFrame = requestAnimationFrame(fadeStep);
    }
  }

  $effect(() => {
    const next = highlight;
    const fade = appSettings.value.viewportLink.fadeSelection;
    void localView;
    untrack(() => {
      updateFade(next, fade);
      if (!pools) return;
      const startedAt = performance.now();
      pools.setHighlight(next);
      highlightStats.applies++;
      highlightStats.lastApply = performance.now() - startedAt;
      updateFocus();
      sceneComponent?.invalidate();
    });
  });

  let gizmoMode = $state<GizmoMode>('translate');
  const gizmoModes: { mode: GizmoMode; label: string; icon: string }[] = [
    { mode: 'translate', label: 'Move', icon: 'i-[tabler--arrows-move]' },
    { mode: 'scale', label: 'Scale', icon: 'i-[tabler--arrows-maximize]' }
  ];

  export const update = function update(result: Int32Array) {
    perf.addPoint('split-result');
    const inputs = splitNestedArray(result);
    perf.endPoint();
    items = inputs;
    links = undefined;
    creators = undefined;

    perf.addPoint('update-geometries');

    const { totalVertices, totalFaces } = updateGeometries(inputs, scene);
    perf.endPoint();

    perf.addPoint('total-vertices', totalVertices);
    perf.addPoint('total-faces', totalFaces);
    updateFocus();
    sceneComponent?.invalidate();

    return {
      vertices: totalVertices,
      faces: totalFaces,
      bounds: bounds.isEmpty()
        ? undefined
        : {
          min: bounds.min.toArray() as [number, number, number],
          max: bounds.max.toArray() as [number, number, number]
        }
    };
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
      {projectId}
      contentBounds={bounds}
      {debugData}
      {gizmos}
      {gizmoMode}
      {ongizmochange}
      {ongizmostart}
      {ongizmoend}
      {picking}
      {pickFormat}
      pickables={() => pools?.pickables() ?? []}
      onpick={handlePick}
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
