<script lang="ts">
  interface Props {
    options?: string[];
    value?: number;
    id?: string;
  }

  let { options = [], value = $bindable(0), id = '' }: Props = $props();
</script>

<select {id} bind:value>
  {#each options as label, i (label)}
    <option value={i}>{label}</option>
  {/each}
</select>

<style>
  select {
    background: var(--layer-2);
    color: var(--text-color);
    font-family: var(--font-family);
    outline: solid 1px var(--outline);
    padding: 0.8em 1em;
    border-radius: 5px;
    border: none;
  }
</style>
