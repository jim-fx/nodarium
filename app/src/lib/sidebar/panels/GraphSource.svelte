<script lang="ts">
  import type { Graph } from '$lib/types';
  import { JsonViewer } from '@nodarium/ui';

  const { graph }: { graph?: Graph } = $props();

  const data = $derived(
    graph
      ? {
        ...graph,
        nodes: graph.nodes.map((n: object) => ({ ...n, tmp: undefined, state: undefined }))
      }
      : null
  );
</script>

<div class="overflow-auto p-2">
  {#if data}
    <JsonViewer value={data} path="graph" />
  {:else}
    <span class="font-mono text-xs text-neutral-500">No graph loaded</span>
  {/if}
</div>
