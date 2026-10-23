<script lang="ts">
  import type { Graph } from '@nodarium/types';
  import { Button, InputSelect, toast } from '@nodarium/ui';
  import { onMount } from 'svelte';

  const ENDPOINT = '/__dev/templates';

  const { graph }: { graph?: Graph } = $props();

  let names = $state<string[]>([]);
  let selected = $state(0);
  let newName = $state('');
  const isNew = $derived(selected === names.length);
  let pending = $state(false);

  onMount(async () => {
    const res = await fetch(ENDPOINT);
    if (res.ok) names = await res.json();
  });

  async function push() {
    const name = isNew ? newName.trim() : names[selected];
    if (!graph || !name) return;
    pending = true;
    try {
      const res = await fetch(ENDPOINT, {
        method: 'POST',
        body: JSON.stringify({ name, graph })
      });
      if (!res.ok) throw new Error((await res.json()).error);
      const { created } = await res.json();
      if (created) names = [...names, name].sort();
      selected = names.indexOf(name);
      newName = '';
      toast(`${created ? 'Created' : 'Wrote'} ${name}.json`, 'success');
    } catch (e) {
      toast(`Could not write template: ${e instanceof Error ? e.message : e}`, 'error');
    }
    pending = false;
  }
</script>

<div class="flex flex-col gap-2 p-2">
  <InputSelect options={[...names, 'New template…']} bind:value={selected} />
  {#if isNew}
    <input
      class="bg-layer-2 text-text border-outline rounded border px-2 py-1"
      placeholder="template-name"
      bind:value={newName}
    />
  {/if}
  <Button
    variant="primary"
    disabled={!graph || pending || (isNew && !newName.trim())}
    onclick={push}
  >
    {isNew ? 'Create template' : 'Push graph to template'}
  </Button>
</div>
