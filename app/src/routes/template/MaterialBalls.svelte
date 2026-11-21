<script lang="ts">
  import { getMaterialPreviews } from '#lib/result-viewer/materials/material-previews.ts';
  import { materials } from '#lib/result-viewer/materials/materials.ts';

  type Props = {
    value: string;
    onchange: (id: string) => void;
  };

  const { value, onchange }: Props = $props();

  const previews = getMaterialPreviews();
  let buttons = $state<HTMLButtonElement[]>([]);

  function onkeydown(event: KeyboardEvent, index: number) {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (!step) return;
    event.preventDefault();
    const next = (index + step + materials.length) % materials.length;
    onchange(materials[next].id);
    buttons[next]?.focus();
  }
</script>

<div role="radiogroup" aria-label="Material" class="flex items-center gap-1.5">
  {#each materials as material, i (material.id)}
    {@const selected = value === material.id}
    <button
      bind:this={buttons[i]}
      type="button"
      role="radio"
      aria-checked={selected}
      tabindex={selected ? 0 : -1}
      aria-label={material.name}
      title={material.name}
      class="
        h-8 w-8 shrink-0 cursor-pointer rounded-full border-2 bg-transparent p-0.5
        {selected ? 'border-selected' : 'border-transparent opacity-70 hover:opacity-100'}
      "
      onclick={() => onchange(material.id)}
      onkeydown={(e) => onkeydown(e, i)}
    >
      {#await previews}
        <span class="block h-full w-full rounded-full bg-layer-2"></span>
      {:then images}
        <img src={images[material.id]} alt="" class="block h-full w-full" />
      {/await}
    </button>
  {/each}
</div>
