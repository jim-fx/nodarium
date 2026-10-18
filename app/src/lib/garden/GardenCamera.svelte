<script lang="ts">
  import { T, useTask, useThrelte } from '@threlte/core';
  import { onMount } from 'svelte';
  import { type PerspectiveCamera, Vector3 } from 'three';
  import { BED_DEPTH, BED_WIDTH, PLANT_MAX_HEIGHT } from './config';

  type Props = {
    focus?: { x: number; y: number; z: number; height: number };
    /** receives the point the camera looks at */
    target: Vector3;
    /** the fog starts this far behind the target and is this long */
    fog: { start: number; length: number };
  };
  const { focus, target, fog }: Props = $props();

  const FOV = 30;
  /** The visitor stands in front of the bed and looks down at it */
  const DIRECTION = new Vector3(0, 0.42, 1).normalize();
  const RIGHT = new Vector3(1, 0, 0);
  const UP = new Vector3().crossVectors(DIRECTION, RIGHT);
  const TARGET = new Vector3(0, 0.5, 0);
  /** The corners of the bed and the tallest possible plants at its back */
  const FRAME = [-1, 1].flatMap((x) => [
    new Vector3(x * (BED_WIDTH / 2 + 0.3), 0, BED_DEPTH / 2 + 0.3),
    new Vector3(x * (BED_WIDTH / 2 + 0.3), 0, -BED_DEPTH / 2 - 0.3),
    new Vector3(x * (BED_WIDTH / 2 - 1), PLANT_MAX_HEIGHT, -BED_DEPTH / 2 + 1)
  ]);
  /** Share of the screen the garden may fill */
  const FILL = 0.9;
  /** How far the view follows the pointer, kept small so the garden barely turns */
  const PARALLAX = { x: 0.9, y: 0.35 };
  const DAMPING = 1.8;
  /** On narrow screens the sides of the bed are cut off instead of moving far away */
  const MAX_DISTANCE = 30;

  const { size, scene } = useThrelte();

  let camera = $state<PerspectiveCamera>();
  const pointer = { x: 0, y: 0 };
  const desiredTarget = new Vector3();
  const desiredPosition = new Vector3();

  const relative = new Vector3();
  const overviewTarget = TARGET.clone();
  let overviewDistance = 20;

  /** Finds the closest view in which the whole bed is visible and centered */
  function fitOverview(aspect: number) {
    const tanY = Math.tan((FOV / 2) * (Math.PI / 180)) * FILL;
    const tanX = tanY * aspect;
    overviewTarget.copy(TARGET);
    for (let iteration = 0; iteration < 4; iteration++) {
      let distance = 0;
      for (const point of FRAME) {
        relative.subVectors(point, overviewTarget);
        const depth = relative.dot(DIRECTION);
        distance = Math.max(
          distance,
          Math.abs(relative.dot(RIGHT)) / tanX + depth,
          Math.abs(relative.dot(UP)) / tanY + depth
        );
      }
      // shift the target until the bed is vertically centered on the screen
      let minY = Infinity, maxY = -Infinity;
      for (const point of FRAME) {
        relative.subVectors(point, overviewTarget);
        const y = relative.dot(UP) / (distance - relative.dot(DIRECTION));
        minY = Math.min(minY, y);
        maxY = Math.max(maxY, y);
      }
      overviewTarget.addScaledVector(UP, ((minY + maxY) / 2) * distance);
      overviewDistance = Math.min(distance, MAX_DISTANCE);
    }
  }

  $effect(() => {
    fitOverview($size.width / Math.max($size.height, 1));
  });

  function updateDesired() {
    if (focus) {
      desiredTarget.set(focus.x, focus.y, focus.z);
      const distance = 3.2 + focus.height * 1.6;
      desiredPosition.copy(desiredTarget).addScaledVector(DIRECTION, distance);
    } else {
      desiredTarget.copy(overviewTarget);
      desiredPosition.copy(overviewTarget).addScaledVector(DIRECTION, overviewDistance);
    }
    const parallax = focus ? 0.4 : 1;
    desiredPosition.x += pointer.x * PARALLAX.x * parallax;
    desiredPosition.y += pointer.y * PARALLAX.y * parallax;
  }

  onMount(() => {
    if (!camera) return;
    fitOverview($size.width / Math.max($size.height, 1));
    updateDesired();
    target.copy(desiredTarget);
    camera.position.copy(desiredPosition);
    camera.lookAt(target);
  });

  useTask((delta) => {
    if (!camera) return;
    updateDesired();
    const t = 1 - Math.exp(-delta * DAMPING);
    target.lerp(desiredTarget, t);
    camera.position.lerp(desiredPosition, t);
    camera.lookAt(target);

    if (scene.fog && 'near' in scene.fog) {
      const distance = camera.position.distanceTo(target);
      scene.fog.near = distance + fog.start;
      scene.fog.far = distance + fog.start + fog.length;
    }
  });
</script>

<svelte:window
  onpointermove={(event) => {
    pointer.x = (event.clientX / window.innerWidth) * 2 - 1;
    pointer.y = -((event.clientY / window.innerHeight) * 2 - 1);
  }}
/>

<T.PerspectiveCamera bind:ref={camera} makeDefault fov={FOV} near={0.1} far={120} />
