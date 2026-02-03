<script lang="ts">
  interface Props {
    ctrl?: boolean;
    shift?: boolean;
    alt?: boolean;
    key: string | string[];
  }

  let { ctrl = false, shift = false, alt = false, key }: Props = $props();
</script>

<div class="command bg-layer-2">
  {#if ctrl}
    <span>Ctrl</span>
  {/if}
  {#if shift}
    <span>Shift</span>
  {/if}
  {#if alt}
    <span>Alt</span>
  {/if}
  {key}
</div>

<style>
  .command {
    padding: 0.4em;
    padding-inline: 0.8em;
    font-size: 0.8em;
    border-radius: 0.3em;
    white-space: nowrap;
    width: fit-content;
  }

  span::after {
    content: " +";
    opacity: 0.5;
  }
</style>
