<script lang="ts">
  import type { Snippet } from 'svelte';
  import { setGridId } from './context';

  const { children, id }: { children?: Snippet<[{ id?: string }]>; id?: string } = $props();

  // svelte-ignore state_referenced_locally
  setGridId(id);
</script>

{@render children?.({ id })}
