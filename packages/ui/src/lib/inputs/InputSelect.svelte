<script lang="ts">
  type SelectOption = string | { value: number; label: string };

  interface Props {
    options?: SelectOption[];
    value?: number;
    id?: string;
  }

  let { options = [], value = $bindable(0), id = '' }: Props = $props();

  const normalized = $derived(
    options.map((opt, i) => typeof opt === 'string' ? { value: i, label: opt } : opt)
  );
</script>

<select {id} bind:value class="bg-layer-2 text-text">
  {#each normalized as opt (opt.value)}
    <option value={opt.value}>{opt.label}</option>
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
