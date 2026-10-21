<script lang="ts">
  import { getMaterialPreviews } from '#lib/result-viewer/material-previews.ts';
  import { materials } from '#lib/result-viewer/materials.ts';

  type Props = {
    value: string;
    onchange: (id: string) => void;
  };

  const { value, onchange }: Props = $props();

  const previews = getMaterialPreviews();
</script>

<div class="grid grid-cols-3 gap-2 pr-4">
  {#each materials as material (material.id)}
    {@const selected = value === material.id}
    <button
      class="
        flex flex-col items-center gap-1 p-2 rounded-sm border cursor-pointer
        {selected ? 'border-selected bg-layer-2' : 'border-outline hover:bg-layer-2'}
      "
      aria-pressed={selected}
      onclick={() => onchange(material.id)}
    >
      <div class="w-14 h-14">
        {#await previews then images}
          <img src={images[material.id]} alt="" class="w-full h-full" />
        {/await}
      </div>
      <span class="text-xs text-text">{material.name}</span>
    </button>
  {/each}
</div>
