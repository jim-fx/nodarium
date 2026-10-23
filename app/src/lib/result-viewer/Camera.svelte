<script lang="ts">
  import { T, useTask } from '@threlte/core';
  import { OrbitControls } from '@threlte/extras';
  import { onMount, untrack } from 'svelte';
  import { Vector3 } from 'three';
  import type { PerspectiveCamera, Vector3Tuple } from 'three';
  import type { OrbitControls as OrbitControlsType } from 'three/examples/jsm/controls/OrbitControls.js';
  import { damp, type Framing } from './camera-framing';

  let camera = $state<PerspectiveCamera>();
  let controls = $state<OrbitControlsType>();

  type Props = {
    framing?: Framing;
    /** Bumped to frame the content again, even if it did not change */
    frameRequest?: number;
    centerCamera: boolean;
    projectId?: number;
  };

  const { framing, frameRequest = 0, centerCamera, projectId }: Props = $props();

  type Transform = { camera: Vector3Tuple; target: Vector3Tuple };

  const storageKey = (id: number | undefined) => `nodes.camera.transform.${id ?? 'none'}`;

  function readTransform(id: number | undefined): Transform | undefined {
    try {
      const stored = localStorage.getItem(storageKey(id));
      if (!stored) return undefined;
      const t = JSON.parse(stored) as Transform;
      return [...t.camera, ...t.target].every(Number.isFinite) ? t : undefined;
    } catch {
      return undefined;
    }
  }

  const initial = untrack(() => readTransform(projectId));

  function saveCameraState() {
    if (!camera || !controls) return;
    const cPos = camera.position.toArray();
    const tPos = controls.target.toArray();
    if (tPos.some((v) => isNaN(v)) || cPos.some((v) => isNaN(v))) return;
    try {
      localStorage.setItem(storageKey(projectId), JSON.stringify({ camera: cPos, target: tPos }));
    } catch { /* storage unavailable */ }
  }

  /** How much the content has to grow or shrink before the camera zooms with it */
  const REFIT_RATIO = 1.5;
  const SPEED = 6;

  const goalTarget = new Vector3();
  let goalDistance: number | undefined;
  let lastRadius: number | undefined;
  let lastRequest: number | undefined;
  let animating = false;

  const task = useTask((delta) => {
    if (!controls || !camera || !framing) return;

    const toGoal = goalTarget.clone().sub(controls.target);
    const targetDone = toGoal.length() < framing.radius * 0.002;
    if (!targetDone) {
      controls.target.set(
        damp(controls.target.x, goalTarget.x, SPEED, delta),
        damp(controls.target.y, goalTarget.y, SPEED, delta),
        damp(controls.target.z, goalTarget.z, SPEED, delta)
      );
    }

    let distanceDone = true;
    if (goalDistance !== undefined) {
      const offset = camera.position.clone().sub(controls.target);
      const distance = offset.length();
      distanceDone = Math.abs(distance - goalDistance) < framing.radius * 0.002;
      if (!distanceDone && distance > 0) {
        offset.setLength(damp(distance, goalDistance, SPEED, delta));
        camera.position.copy(controls.target).add(offset);
      }
    }

    controls.update();

    if (targetDone && distanceDone) {
      animating = false;
      task.stop();
    }
  });
  task.stop();

  /** The user grabbed the camera, so the animation must not fight them */
  function stopAnimation() {
    if (!animating) return;
    animating = false;
    task.stop();
  }

  function animateTo(f: Framing, fit: boolean) {
    goalTarget.set(...f.center);
    goalDistance = fit ? f.distance : undefined;
    lastRadius = f.radius;
    if (!animating) {
      animating = true;
      task.start();
    }
  }

  $effect(() => {
    if (!framing || !controls) return;

    const requested = lastRequest !== undefined && frameRequest !== lastRequest;
    lastRequest = frameRequest;
    if (!requested && !centerCamera) return;

    const resized = lastRadius === undefined
      || framing.radius / lastRadius > REFIT_RATIO
      || lastRadius / framing.radius > REFIT_RATIO;
    animateTo(framing, requested || resized);
  });

  // Each project remembers where its camera was, a new one gets framed once its content arrives
  let currentProject = untrack(() => projectId);
  let fitPending = false;
  $effect(() => {
    const id = projectId;
    if (id === currentProject || !camera || !controls) return;
    currentProject = id;
    stopAnimation();
    const t = readTransform(id);
    if (t) {
      camera.position.fromArray(t.camera);
      controls.target.fromArray(t.target);
      controls.update();
      lastRadius = undefined;
      fitPending = false;
    } else {
      fitPending = true;
    }
  });

  $effect(() => {
    if (!fitPending || !framing || !controls) return;
    fitPending = false;
    untrack(() => animateTo(framing, true));
  });

  onMount(() => {
    const t = initial ?? { camera: [10, 10, 10], target: [0, 0, 0] } as Transform;
    camera?.position.fromArray(t.camera);
    controls?.target.fromArray(t.target);
    controls?.update();
  });
</script>

<T.PerspectiveCamera
  bind:ref={camera}
  position={initial?.camera ?? [10, 10, 10]}
  makeDefault
  fov={50}
  near={framing?.near ?? 0.1}
  far={framing?.far ?? 2000}
>
  <OrbitControls
    bind:ref={controls}
    minDistance={framing ? framing.radius * 0.02 : 0}
    maxDistance={framing ? framing.radius * 50 : Infinity}
    on:start={stopAnimation}
    on:change={saveCameraState}
  />
</T.PerspectiveCamera>
