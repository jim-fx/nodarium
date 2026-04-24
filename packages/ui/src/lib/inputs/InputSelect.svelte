<script lang="ts">
  type StringOption = string;
  type LabeledOption = { label: string; value: string };

  interface Props {
    options?: StringOption[] | LabeledOption[];
    value?: number | string;
    id?: string;
  }

  let { options = [], value = $bindable<number | string>(0), id = '' }: Props = $props();

  const isLabeled = $derived(options.length > 0 && typeof options[0] === 'object');
</script>

<select {id} bind:value class="bg-layer-2 text-text">
  {#if isLabeled}
    {#each options as opt ((opt as LabeledOption).value)}
      <option value={(opt as LabeledOption).value}>{(opt as LabeledOption).label}</option>
    {/each}
  {:else}
    {#each options as label, i (label)}
      <option value={i}>{label as string}</option>
    {/each}
  {/if}
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
