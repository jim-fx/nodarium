<script lang="ts">
  import { T } from '@threlte/core';
  import { type OrthographicCamera } from 'three';
  type Props = {
    camera: OrthographicCamera;
    position: [number, number, number];
  };

  let { camera = $bindable(), position }: Props = $props();
</script>

<T.OrthographicCamera
  bind:ref={camera}
  position.x={position[0]}
  position.y={10}
  position.z={position[1]}
  rotation.x={-Math.PI / 2}
  zoom={position[2]}
  makeDefault
/>
