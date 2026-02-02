<script lang="ts">
  import type { Graph } from '$lib/types';

  const { graph }: { graph?: Graph } = $props();

  function convert(g: Graph): string {
    return JSON.stringify(
      {
        ...g,
        nodes: g.nodes.map((n: object) => ({ ...n, tmp: undefined }))
      },
      null,
      2
    );
  }
</script>

<pre>
  {graph ? convert(graph) : "No graph loaded"}
</pre>
