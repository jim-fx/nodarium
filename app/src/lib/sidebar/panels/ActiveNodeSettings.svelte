<script lang="ts">
  import type { GraphManager } from '$lib/graph-interface/graph-manager.svelte';
  import type { NodeInstance } from '@nodarium/types';
  import ActiveNodeSelected from './ActiveNodeSelected.svelte';

  type Props = {
    manager: GraphManager;
    node: NodeInstance | undefined;
  };

  let { manager, node = $bindable() }: Props = $props();
</script>

<div class='{node?"border-l-2 pl-3.5!":""} bg-layer-2 flex items-center h-[70px] border-b-1 border-l-selected border-b-outline pl-4'>
  <h3>Node Settings</h3>
</div>

{#if node}
  {#key node.id}
    {#if node}
      <ActiveNodeSelected {manager} bind:node />
    {/if}
  {/key}
{:else}
  <p class="mx-4 mt-4">No node selected</p>
{/if}
