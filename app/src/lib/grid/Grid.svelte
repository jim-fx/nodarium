<script lang="ts">
  import { onMount, setContext, type Snippet } from 'svelte';

  const { children, id }: { children?: Snippet<[{ id?: string }]>; id?: string } = $props();

  onMount(() => {
    setContext('grid-id', id);
  });
</script>

{@render children?.({ id })}
