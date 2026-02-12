<script lang="ts">
  import { T } from '@threlte/core';
  import type { Group } from 'three';
  import { updateDebugScene } from './debug';

  type Props = {
    debugData?: Record<number, { type: string; data: Int32Array }>;
  };

  let group = $state<Group>(null!);
  const { debugData }: Props = $props();

  $effect(() => {
    if (!group || !debugData) return;
    updateDebugScene(group, $state.snapshot(debugData));
  });
</script>

<T.Group bind:ref={group} />
