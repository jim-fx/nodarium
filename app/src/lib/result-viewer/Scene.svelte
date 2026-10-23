<script lang="ts">
  import { colors } from '#lib/graph-interface/graph/colors.svelte.ts';
  import { unitSettings } from '@nodarium/ui';
  import { T, useTask, useThrelte } from '@threlte/core';
  import { Environment, Grid } from '@threlte/extras';
  import { untrack } from 'svelte';
  import { Box3, type Group, Vector3 } from 'three';
  import { appSettings } from '../settings/app-settings.svelte';
  import { frameBounds, type Framing, gridSectionSize } from './camera/camera-framing';
  import Camera from './camera/Camera.svelte';
  import Debug from './debug/Debug.svelte';
  import type { Gizmo, GizmoChange, GizmoMode } from './gizmos/gizmos';
  import Gizmos from './gizmos/Gizmos.svelte';
  import ScaleModel from './scale/ScaleModel.svelte';
  import ScaleViewer from './scale/ScaleViewer.svelte';
  import { updateViewScale } from './scale/view-scale.svelte';

  const { renderStage, invalidate: _invalidate, size } = useThrelte();

  type Props = {
    fps: number[];
    debugData?: Record<number, { type: string; data: Int32Array }>;
    scene: Group;
    centerCamera: boolean;
    projectId?: number;
    /** the bounds of everything the pools show, they change with every update */
    contentBounds: Box3;
    gizmos?: Gizmo[];
    gizmoMode?: GizmoMode;
    ongizmochange?: (nodeId: number, change: GizmoChange) => void;
    ongizmostart?: () => void;
    ongizmoend?: () => void;
  };

  let {
    centerCamera,
    projectId,
    contentBounds,
    fps = $bindable(),
    scene = $bindable(),
    debugData,
    gizmos = [],
    gizmoMode = 'translate',
    ongizmochange,
    ongizmostart,
    ongizmoend
  }: Props = $props();

  let framing = $state<Framing>();
  let frameRequest = $state(0);
  let bounds = $state.raw<Box3>();

  const gridSection = $derived(gridSectionSize(framing?.radius ?? 4));

  const aspect = $derived($size.height > 0 ? $size.width / $size.height : 1);

  // a narrower viewer needs more distance to keep the plant in view
  $effect(() => {
    void aspect;
    untrack(measure);
  });

  function measure() {
    const aabb = contentBounds.clone();
    // a single geometry with NaN positions would move the camera to NaN
    const next = frameBounds(aabb.min.toArray(), aabb.max.toArray(), 50, aspect);
    if (next) {
      bounds = aabb;
      framing = next;
      updateViewScale(next.radius);
    }
  }

  // lengths changed their scale, so the content has to be looked at again
  $effect(() => {
    void unitSettings.unit;
    untrack(() => {
      measure();
      frameRequest++;
    });
  });

  useTask(
    (delta) => {
      fps.push(1 / delta);
      fps = fps.slice(-100);
    },
    { stage: renderStage, autoInvalidate: false }
  );

  export const invalidate = function() {
    measure();
    _invalidate();
  };
</script>

<Environment url="/environments/meadow_1k.exr" />

<T.DirectionalLight intensity={0.5} position={[0, 0, 1]} />
<T.AmbientLight intensity={1} />

{#if appSettings.value.showScaleModel}
  <ScaleModel {bounds} />
{/if}

{#if appSettings.value.showScaleViewer}
  <ScaleViewer {bounds} />
{/if}

<Camera {framing} {frameRequest} {centerCamera} {projectId} />

<Debug {debugData} />

<Gizmos
  {gizmos}
  mode={gizmoMode}
  onchange={ongizmochange}
  onstart={ongizmostart}
  onend={ongizmoend}
/>

{#if appSettings.value.showGrid}
  <Grid
    cellColor={colors['outline']}
    cellThickness={0.7}
    cellSize={gridSection / 2}
    sectionSize={gridSection}
    infiniteGrid
    sectionThickness={0.7}
    sectionDistance={2 * gridSection}
    sectionColor={colors['outline']}
    fadeDistance={(framing?.radius ?? 4) * 12}
    fadeStrength={10}
    fadeOrigin={new Vector3(0, 0, 0)}
  />
{/if}
<T.Group bind:ref={scene}></T.Group>
