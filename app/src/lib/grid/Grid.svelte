<script lang="ts">
  import { onMount, setContext, type Snippet } from 'svelte';

  const { children, id } = $props<{ children?: Snippet; id?: string }>();

  onMount(() => {
    setContext('grid-id', id);
  });
</script>

{@render children({ id })}
