<script lang="ts">
  import type { Graph } from '@nodarium/types';
  import { Button, InputSelect, toast } from '@nodarium/ui';
  import { onMount, untrack } from 'svelte';
  import { matchTemplate, nodeSignatures, type TemplateSummary } from './template-match';

  const ENDPOINT = '/__dev/templates';

  const { graph }: { graph?: Graph } = $props();

  let templates = $state<TemplateSummary[]>([]);
  let loaded = $state(false);
  const names = $derived(templates.map((t) => t.name));
  let newName = $state('');
  let pending = $state(false);

  const graphId = $derived(graph?.id);

  // the template the open project came from, picked again when another project is opened;
  // choosing another one in the select overrides it until then. Without a match it offers a new
  // template, so an unrelated graph doesn't overwrite the first one by accident.
  let selected = $derived.by(() => {
    void graphId;
    if (!loaded) return 0;
    return untrack(() => {
      const match = graph && matchTemplate(graph, templates);
      return match ? names.indexOf(match) : names.length;
    });
  });
  const isNew = $derived(loaded && selected === names.length);

  onMount(async () => {
    const res = await fetch(ENDPOINT);
    if (res.ok) templates = await res.json();
    loaded = true;
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
      const summary = { name, nodes: nodeSignatures(graph) };
      templates = [...templates.filter((t) => t.name !== name), summary].sort((a, b) =>
        a.name.localeCompare(b.name)
      );
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
