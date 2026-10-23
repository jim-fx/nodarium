<script lang="ts">
  import localStore from '#lib/helpers/localStore.ts';
  import { T, useTask } from '@threlte/core';
  import { OrbitControls } from '@threlte/extras';
  import { onMount } from 'svelte';
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
  };

  const { framing, frameRequest = 0, centerCamera }: Props = $props();

  const cameraTransform = localStore<{
    camera: Vector3Tuple;
    target: Vector3Tuple;
  }>('nodes.camera.transform', {
    camera: [10, 10, 10],
    target: [0, 0, 0]
  });

  function saveCameraState() {
    if (!camera || !controls) return;
    let cPos = camera.position.toArray();
    let tPos = controls.target.toArray();
    // check if tPos is NaN or tPos is NaN
    if (tPos.some((v) => isNaN(v)) || cPos.some((v) => isNaN(v))) return;
    $cameraTransform = {
      camera: cPos,
      target: tPos
    };
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

  onMount(() => {
    controls?.target.fromArray($cameraTransform.target);
    controls?.update();
  });
</script>

<T.PerspectiveCamera
  bind:ref={camera}
  position={$cameraTransform.camera}
  makeDefault
  fov={50}
  near={framing?.near ?? 0.1}
  far={framing?.far ?? 2000}
>
  <OrbitControls
    bind:ref={controls}
    minDistance={framing ? framing.radius * 0.02 : 0}
    maxDistance={framing ? framing.radius * 50 : Infinity}
    on:change={saveCameraState}
  />
</T.PerspectiveCamera>
