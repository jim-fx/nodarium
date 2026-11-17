<script lang="ts">
  import { T } from '@threlte/core';
  type Props = {
    position: [number, number, number];
  };

  let { position }: Props = $props();
</script>

<T.OrthographicCamera
  position.x={position[0]}
  position.y={10}
  position.z={position[1]}
  rotation.x={-Math.PI / 2}
  zoom={position[2]}
  makeDefault
/>
