<script lang="ts">
  interface Props {
    ctrl?: boolean;
    shift?: boolean;
    alt?: boolean;
    key: string | string[];
  }

  let { ctrl = false, shift = false, alt = false, key }: Props = $props();
</script>

<div class="command">
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
    background: var(--layer-2);
    padding: 0.4em;
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
