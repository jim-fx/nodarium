<script lang="ts">
  interface Props {
    title?: string;
    transparent?: boolean;
    children?: import('svelte').Snippet;
    open?: boolean;
  }

  let { title = 'Details', transparent = false, children, open = $bindable(false) }: Props =
    $props();
</script>

<details class:transparent bind:open>
  <summary>{title}</summary>
  <div class="content">
    {@render children?.()}
  </div>
</details>

<style>
  details {
    padding: 1em;
    color: var(--text-color);
    padding-left: 20px;
    background-color: #202020;
    outline: solid 0.1px white;
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
