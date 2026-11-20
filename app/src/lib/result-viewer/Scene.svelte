<script lang="ts">
  import { colors } from '#lib/theme/colors.svelte.ts';
  import { unitSettings } from '@nodarium/ui';
  import { T, useTask, useThrelte } from '@threlte/core';
  import { Environment, Grid } from '@threlte/extras';
  import { onDestroy, untrack } from 'svelte';
  import { Box3, type Group, type PerspectiveCamera, Vector2, Vector3 } from 'three';
  import { appSettings } from '../settings/app-settings.svelte';
  import {
    type CameraView,
    frameBounds,
    type Framing,
    gridSectionSize
  } from './camera/camera-framing';
  import Camera from './camera/Camera.svelte';
  import type { Gizmo, GizmoChange, GizmoMode } from './gizmos/gizmos';
  import Gizmos from './gizmos/Gizmos.svelte';
  import { createLights, lighting } from './materials/lighting';
  import { usesEnvironment } from './materials/materials';
  import { createPicker, type Pick, type Pickable, type PickFormat } from './provenance/picker';
  import ScaleModel from './scale/ScaleModel.svelte';
  import ScaleViewer from './scale/ScaleViewer.svelte';
  import { updateViewScale } from './scale/view-scale.svelte';

  const {
    renderStage,
    invalidate: _invalidate,
    size,
    scene: threeScene,
    renderer,
    camera: activeCamera,
    dom
  } = useThrelte();

  const lights = createLights();
  threeScene.environmentIntensity = lighting.environment.intensity;

  type Props = {
    fps: number[];
    /** what the debug node shows, drawn next to the result */
    debugScene: Group;
    scene: Group;
    centerCamera: boolean;
    projectId?: number;
    materialId: string;
    /** the bounds of everything the pools show, they change with every update */
    contentBounds: Box3;
    gizmos?: Gizmo[];
    gizmoMode?: GizmoMode;
    ongizmochange?: (nodeId: number, change: GizmoChange) => void;
    ongizmostart?: () => void;
    ongizmoend?: () => void;
    /** hovering and clicking the result tells what is under the pointer */
    picking?: boolean;
    pickables?: () => Pickable[];
    onpick?: (pick: Pick | undefined, kind: 'hover' | 'click', event?: PointerEvent) => void;
  };

  let {
    centerCamera,
    projectId,
    materialId,
    contentBounds,
    fps = $bindable(),
    scene = $bindable(),
    debugScene,
    gizmos = [],
    gizmoMode = 'translate',
    ongizmochange,
    ongizmostart,
    ongizmoend,
    picking = false,
    pickables,
    onpick
  }: Props = $props();

  let cameraComponent = $state<ReturnType<typeof Camera>>();
  let focusBounds: Box3 | undefined;
  let viewBeforeFocus: CameraView | undefined;

  /**
   * The part of the content the camera frames instead of everything, for the local view. The view
   * the camera had before comes back when the focus is taken away.
   */
  export function setFocus(next: Box3 | undefined) {
    const entering = !!next && !focusBounds;
    const leaving = !next && !!focusBounds;
    focusBounds = next;
    if (entering) viewBeforeFocus = cameraComponent?.snapshot();
    measure();
    if (entering) frameRequest++;
    if (leaving && viewBeforeFocus) {
      cameraComponent?.restore(viewBeforeFocus);
      viewBeforeFocus = undefined;
    }
    _invalidate();
  }

  let picker: ReturnType<typeof createPicker> | undefined;
  let usedFormat: PickFormat = 'integer';
  onDestroy(() => picker?.dispose());

  let pending: { x: number; y: number; click?: PointerEvent } | undefined;
  let pickFrame = 0;
  let pointerInside = false;
  let down: { x: number; y: number } | undefined;
  let hovered: Pick | undefined;

  function emitHover(pick: Pick | undefined) {
    if (pick?.object === hovered?.object && pick?.index === hovered?.index) return;
    hovered = pick;
    onpick?.(pick, 'hover');
  }

  function requestPick(x: number, y: number, click?: PointerEvent) {
    // a click is not replaced by the hover that follows it
    if (pending?.click && !click) return;
    pending = { x, y, click };
    if (!pickFrame && !picker?.busy) pickFrame = requestAnimationFrame(runPick);
  }

  async function runPick() {
    pickFrame = 0;
    const request = pending;
    if (!request || !pickables || !scene) return;
    pending = undefined;
    picker ??= createPicker(renderer, usedFormat);
    const el = renderer.domElement;
    let pick: Pick | undefined;
    try {
      pick = await picker.pick(
        scene,
        $activeCamera,
        pickables(),
        request.x,
        request.y,
        el.clientWidth,
        el.clientHeight
      );
    } catch (error) {
      // an integer target that can't be drawn to or read back, the ids fit into bytes as well
      if (usedFormat === 'bytes') throw error;
      console.warn('picking falls back to an RGBA8 target', error);
      picker.dispose();
      picker = undefined;
      usedFormat = 'bytes';
      return;
    }
    if (request.click) onpick?.(pick, 'click', request.click);
    else if (pointerInside && !down) emitHover(pick);
    if (pending && !pickFrame) pickFrame = requestAnimationFrame(runPick);
  }

  const CLICK_DISTANCE = 4;

  /** Where the pointer is on the canvas, in CSS pixels */
  function canvasPosition(event: PointerEvent) {
    const rect = renderer.domElement.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  }

  // the element belongs to threlte, so the listeners can't be put on it in the markup. It is the
  // one the orbit controls capture the pointer on, the canvas inside it would miss the release.
  $effect(() => {
    if (!picking) return;
    const el = dom;
    const move = (event: PointerEvent) => {
      pointerInside = true;
      // a pressed button orbits the camera or drags a gizmo
      if (event.buttons !== 0) return;
      const { x, y } = canvasPosition(event);
      requestPick(x, y);
    };
    const leave = () => {
      pointerInside = false;
      if (!pending?.click) pending = undefined;
      emitHover(undefined);
    };
    const press = (event: PointerEvent) => {
      down = event.button === 0 ? canvasPosition(event) : undefined;
      emitHover(undefined);
    };
    const release = (event: PointerEvent) => {
      const from = down;
      down = undefined;
      if (!from || event.button !== 0) return;
      const { x, y } = canvasPosition(event);
      if (Math.hypot(x - from.x, y - from.y) > CLICK_DISTANCE) return;
      requestPick(x, y, event);
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerleave', leave);
    el.addEventListener('pointerdown', press);
    el.addEventListener('pointerup', release);
    return () => {
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerleave', leave);
      el.removeEventListener('pointerdown', press);
      el.removeEventListener('pointerup', release);
      pending = undefined;
      emitHover(undefined);
    };
  });

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
    const aabb = (focusBounds && !focusBounds.isEmpty() ? focusBounds : contentBounds).clone();
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

  let lastFrame = 0;
  useTask(
    (delta) => {
      lastFrame = performance.now();
      fps.push(1 / delta);
      fps = fps.slice(-100);
    },
    { stage: renderStage, autoInvalidate: false }
  );

  /** A page that is not visible, or a browser driven by a tool, may give no frames at all */
  const DRAW_WITHOUT_FRAME_MS = 500;
  let drawTimer: ReturnType<typeof setTimeout> | undefined;

  // the size and the aspect are set in a frame too, a page that never had one doesn't have them
  function drawWithoutFrame() {
    const { width, height } = size.current;
    if (width <= 0 || height <= 0) return;
    const drawn = renderer.getSize(new Vector2());
    if (drawn.x !== width || drawn.y !== height) renderer.setSize(width, height);
    const view = activeCamera.current as PerspectiveCamera;
    if (view.isPerspectiveCamera && view.aspect !== width / height) {
      view.aspect = width / height;
      view.updateProjectionMatrix();
    }
    renderer.render(threeScene, view);
  }

  onDestroy(() => clearTimeout(drawTimer));

  /** Draws the scene again as it is, the content did not change */
  export function redraw() {
    _invalidate();
  }

  export const invalidate = function() {
    measure();
    _invalidate();
    const asked = performance.now();
    clearTimeout(drawTimer);
    drawTimer = setTimeout(() => {
      if (lastFrame < asked) drawWithoutFrame();
    }, DRAW_WITHOUT_FRAME_MS);
  };
</script>

{#if usesEnvironment(materialId)}
  <Environment url={lighting.environment.url} />
{/if}

{#each lights as light (light.id)}
  <T is={light} />
{/each}

{#if appSettings.value.showScaleModel}
  <ScaleModel {bounds} />
{/if}

{#if appSettings.value.showScaleViewer}
  <ScaleViewer {bounds} />
{/if}

<Camera bind:this={cameraComponent} {framing} {frameRequest} {centerCamera} {projectId} />

<T is={debugScene} />

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
