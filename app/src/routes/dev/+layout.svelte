<script lang="ts">
  import type { Snippet } from 'svelte';
  const { children }: { children?: Snippet } = $props();
</script>

<main class="w-screen overflow-x-hidden">
  {@render children?.()}
</main>
