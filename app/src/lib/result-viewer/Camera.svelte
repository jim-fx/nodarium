<script lang="ts">
  import localStore from '$lib/helpers/localStore';
  import { T, useTask } from '@threlte/core';
  import { OrbitControls } from '@threlte/extras';
  import { onMount } from 'svelte';
  import { Vector3 } from 'three';
  import type { PerspectiveCamera, Vector3Tuple } from 'three';
  import type { OrbitControls as OrbitControlsType } from 'three/examples/jsm/controls/OrbitControls.js';

  let camera = $state<PerspectiveCamera>();
  let controls = $state<OrbitControlsType>();

  type Props = {
    center: Vector3;
    centerCamera: boolean;
  };

  const { center, centerCamera }: Props = $props();

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

  let isRunning = false;
  const task = useTask(() => {
    if (!controls) return;
    let length = center.clone().sub(controls.target).length();
    if (length < 0.01 || !centerCamera) {
      isRunning = false;
      task.stop();
      return;
    }

    controls.target.lerp(center, 0.02);
    controls.update();
  });
  task.stop();

  $effect(() => {
    if (
      center
      && controls
      && centerCamera
      && (center.x !== controls.target.x
        || center.y !== controls.target.y
        || center.z !== controls.target.z)
      && !isRunning
    ) {
      isRunning = true;
      task.start();
    }
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
>
  <OrbitControls bind:ref={controls} on:change={saveCameraState} />
</T.PerspectiveCamera>
