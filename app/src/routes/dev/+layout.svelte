<script lang="ts">
  import type { Snippet } from 'svelte';
  const { children } = $props<{ children?: Snippet }>();
</script>

<main class="w-screen h-screen overflow-x-hidden">
  {@render children()}
</main>
