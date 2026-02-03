<script lang="ts">
  import type { Snippet } from 'svelte';
  interface Props {
    title?: string;
    transparent?: boolean;
    children?: Snippet;
    open?: boolean;
  }
  let { title = 'Details', transparent = false, children, open = $bindable(false) }: Props =
    $props();
</script>

<details class:transparent bind:open class="text-text outline-1 outline-outline bg-layer-1">
  <summary>{title}</summary>
  <div class="content">
    {@render children?.()}
  </div>
</details>

<style>
  details {
    padding: 1em;
    padding-left: 20px;
    border-radius: 2px;
    font-weight: 300;
    font-size: 0.9em;
  }

  details.transparent {
    background-color: transparent;
    padding: 0;
    outline: none;
  }
</style>
