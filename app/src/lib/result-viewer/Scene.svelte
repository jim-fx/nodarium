<script lang="ts">
  import { colors } from '#lib/graph-interface/graph/colors.svelte.ts';
  import { unitSettings } from '@nodarium/ui';
  import { T, useTask, useThrelte } from '@threlte/core';
  import { Environment, Grid } from '@threlte/extras';
  import { untrack } from 'svelte';
  import { Box3, type BufferGeometry, type Group, Mesh, Vector3 } from 'three';
  import { appSettings } from '../settings/app-settings.svelte';
  import { frameBounds, type Framing, gridSectionSize } from './camera-framing';
  import Camera from './Camera.svelte';
  import Debug from './Debug.svelte';
  import type { Gizmo, GizmoChange, GizmoMode } from './gizmos';
  import Gizmos from './Gizmos.svelte';
  import ScaleModel from './ScaleModel.svelte';
  import ScaleViewer from './ScaleViewer.svelte';
  import { updateViewScale } from './view-scale.svelte';

  const { renderStage, invalidate: _invalidate } = useThrelte();

  type Props = {
    fps: number[];
    debugData?: Record<number, { type: string; data: Int32Array }>;
    scene: Group;
    centerCamera: boolean;
    gizmos?: Gizmo[];
    gizmoMode?: GizmoMode;
    ongizmochange?: (nodeId: number, change: GizmoChange) => void;
    ongizmostart?: () => void;
    ongizmoend?: () => void;
  };

  let {
    centerCamera,
    fps = $bindable(),
    scene = $bindable(),
    debugData,
    gizmos = [],
    gizmoMode = 'translate',
    ongizmochange,
    ongizmostart,
    ongizmoend
  }: Props = $props();

  let geometries = $state.raw<BufferGeometry[]>([]);
  let framing = $state<Framing>();
  let frameRequest = $state(0);
  let bounds = $state.raw<Box3>();

  const gridSection = $derived(gridSectionSize(framing?.radius ?? 4));

  function measure() {
    if (!scene) return;
    const aabb = new Box3().setFromObject(scene);
    // a single geometry with NaN positions would move the camera to NaN
    const next = frameBounds(aabb.min.toArray(), aabb.max.toArray());
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
    if (scene) {
      const geos: BufferGeometry[] = [];
      scene.traverse(function(child) {
        if (isMesh(child)) {
          geos.push(child.geometry);
        }
      });
      geometries = geos;
    }

    if (geometries && scene) measure();
    _invalidate();
  };

  function isMesh(child: unknown): child is Mesh {
    return (
      child !== null
      && typeof child === 'object'
      && 'isObject3D' in child
      && child.isObject3D === true
      && 'material' in child
    );
  }
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

<Camera {framing} {frameRequest} {centerCamera} />

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
