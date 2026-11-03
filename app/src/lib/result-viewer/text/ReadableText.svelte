<script lang="ts">
  import { T, useTask, useThrelte } from '@threlte/core';
  import { Text } from '@threlte/extras';
  import type { ComponentProps } from 'svelte';
  import { type Group, Quaternion, Vector3 } from 'three';
  import { type Flip, flipRotation, NO_FLIP, pickFlip } from './readable-text';

  /**
   * Text that stays a real object in the scene, but turns around in half steps
   * so it is never seen mirrored or upside down. It flips around its own origin,
   * so it is centered vertically by default.
   */
  const { anchorY = 'middle', ...props }: ComponentProps<typeof Text> = $props();

  const { camera } = useThrelte();

  let base = $state<Group>();
  let flip = $state<Flip>(NO_FLIP);

  const orientation = new Quaternion();
  const position = new Vector3();
  const cameraPosition = new Vector3();
  const cameraUp = new Vector3();

  useTask(
    () => {
      const cam = camera.current;
      if (!base) return;
      base.getWorldQuaternion(orientation);
      base.getWorldPosition(position);
      cam.getWorldPosition(cameraPosition);
      cameraUp.set(0, 1, 0).applyQuaternion(cam.getWorldQuaternion(new Quaternion()));
      flip = pickFlip(orientation, position, cameraPosition, cameraUp, flip);
    },
    { autoInvalidate: false }
  );
</script>

<T.Group bind:ref={base}>
  <T.Group rotation={flipRotation(flip)}>
    <Text {anchorY} {...props} />
  </T.Group>
</T.Group>
