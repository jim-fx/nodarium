<script lang="ts">
  import { T } from '@threlte/core';
  import type { Group } from 'three';
  import { type DebugEntry, updateDebugScene } from './debug';

  type Props = {
    debugData?: Record<number, DebugEntry>;
  };

  let group = $state<Group>(null!);
  const { debugData }: Props = $props();

  $effect(() => {
    if (!group || !debugData) return;
    updateDebugScene(group, $state.snapshot(debugData));
  });
</script>

<T.Group bind:ref={group} />
