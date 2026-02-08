<script lang="ts">
  import { type Snippet } from 'svelte';
  interface Props {
    title?: string;
    transparent?: boolean;
    children?: Snippet;
    open?: boolean;
    class?: string;
  }
  let { title = 'Details', transparent = false, children, open = $bindable(false), class: _class }:
    Props = $props();
</script>

<details
  class:transparent
  bind:open
  class="text-text outline-1 outline-outline bg-layer-1 {_class}"
>
  <summary>{title}</summary>
  <div>
    {@render children?.()}
  </div>
</details>

<style>
  details {
    border-radius: 2px;
  }
  summary {
    padding: 1em;
    padding-left: 20px;
    font-weight: 300;
    font-size: 0.9em;
  }

  details[open] > summary {
    border-bottom: solid thin var(--color-outline);
  }

  details > div {
    padding: 1em;
  }

  details.transparent {
    background-color: transparent;
    padding: 0;
    outline: none;
  }
</style>
