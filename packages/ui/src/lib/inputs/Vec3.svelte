<script lang="ts">
  import Float from './Float.svelte';

  interface Props {
    value?: number[];
    id?: string;
  }

  let { value = $bindable([0, 0, 0]), id = '' }: Props = $props();
</script>

<div>
  <Float id={`${id}-x`} bind:value={value[0]} step={0.01} />
  <Float id={`${id}-y`} bind:value={value[1]} step={0.01} />
  <Float id={`${id}-z`} bind:value={value[2]} step={0.01} />
</div>

<style>
  div > :global(.component-wrapper:nth-child(1)) {
    border-radius: 4px 4px 0px 0px !important;
    border-bottom: none !important;
  }
  div > :global(.component-wrapper:nth-child(2)) {
    border-radius: 0px !important;
    outline: none;
    border: solid thin var(--outline);
    border-top: solid thin color-mix(in srgb, var(--outline) 50%, transparent);
    border-bottom: solid thin color-mix(in srgb, var(--outline) 50%, transparent);
  }
  div > :global(.component-wrapper:nth-child(3)) {
    border-top: none !important;
    border-radius: 0px 0px 4px 4px !important;
  }
</style>
