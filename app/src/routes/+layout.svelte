<script lang="ts">
  import '@nodarium/ui/app.css';
  import '../app.css';
  import * as config from '$lib/config';
  import type { Snippet } from 'svelte';
  const { children } = $props<{ children?: Snippet }>();
</script>

{@render children?.()}

<svelte:head>
  {#if config.ANALYTIC_SCRIPT}
    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
    {@html config.ANALYTIC_SCRIPT}
  {/if}
</svelte:head>
