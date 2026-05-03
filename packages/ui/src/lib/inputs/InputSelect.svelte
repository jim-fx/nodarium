<script lang="ts">
  interface Props {
    options?: string[];
    value?: number;
    id?: string;
  }

  let { options = [], value = $bindable(0), id = '' }: Props = $props();

  $effect(() => {
    console.log({ options, value });
    if (typeof value !== typeof options[0]) {
      console.trace('WARNING: value type does not match options type');
    }
  });
</script>

<select {id} bind:value class="bg-layer-2 text-text">
  {#each options as label, i (label)}
    <option value={i}>{label}</option>
  {/each}
</select>

<style>
  select {
    font-family: var(--font-family);
    outline: solid 1px var(--color-outline);
    padding: 0.5em 0.8em;
    border-radius: 5px;
    border: none;
  }
</style>
