<script lang="ts">
  import type { NodeInput } from '@nodarium/types';
  type Props = {
    inputs?: Record<string, NodeInput>;
    colors: Record<string, string>;
    onremove?: (key: string) => void;
    types: string[];
  };
  let { inputs = $bindable(), onremove, colors = {}, types = ['seed', 'float', 'path'] }: Props =
    $props();

  let potentialRow = $state<
    {
      type: string;
      label: string;
    } | undefined
  >();

  function showPotentialRow() {
    potentialRow = {
      type: types[0],
      label: 'Input ' + Object.keys(inputs ?? {}).length
    };
  }

  function realizePotentialRow() {
    if (inputs) inputs[`input_${Object.keys(inputs).length}`] = potentialRow as NodeInput;
    potentialRow = undefined;
  }

  function removeRow(key?: string) {
    if (!key) {
      potentialRow = undefined;
    } else if (inputs) {
      onremove?.(key);
    }
  }

  function getColor(type: string) {
    if (type in colors) {
      return colors[type];
    }

    return '#f00';
  }
</script>

{#snippet row(input: { type: string; label?: string }, remove: () => void, add?: () => void)}
  <div class="flex min-w-0">
    <span
      style:background={getColor(input.type)}
      data-type={input.type}
      class="block opacity-50 min-w-2 ml-2 w-2 h-2 my-auto rounded-sm"
    ></span>
    <select
      class="text-[0.9em] border-r w-19 shrink-0 px-2 py-1 border-outline"
      bind:value={input.type}
    >
      {#each types as type (type)}
        <option>
          <span
            style="background: {getColor(type)}; width: 5px; height: 5px; border-radius: 5px;"
          ></span>
          {type}
        </option>
      {/each}
    </select>
    <input
      class="px-2 grow min-w-30 border-r border-outline text-[0.9em]"
      type="text"
      bind:value={input.label}
    />
    <button
      class="px-2 cursor-pointer opacity-50 hover:opacity-100 hover:bg-red-400"
      onclick={remove}
      aria-label="remove"
    >
      {#if add}
        <span class="py-1 block i-[tabler--cancel]"></span>
      {:else}
        <span class="py-1 block i-[tabler--trash]"></span>
      {/if}
    </button>
    {#if add}
      <button
        class="px-2 border-l hover:bg-green-300 opacity-50 hover:opacity-100 hover:text-layer-1 border-outline cursor-pointer"
        onclick={add}
        aria-label="add"
      >
        <span class="py-1 block i-[tabler--circle-plus]"></span>
      </button>
    {/if}
  </div>
{/snippet}

<div class="rounded-sm overflow-hidden bg-layer-2 divide-y divide-outline outline-1 outline-outline">
  {#each Object.entries(inputs ?? {}) as [key, input] (key)}
    {@render row(input, () => removeRow(key))}
  {/each}
  {#if potentialRow}
    <div class="opacity-80">
      {@render row(potentialRow, () => removeRow(), () => realizePotentialRow())}
    </div>
  {:else}
    <div class="opacity-40">
      <div class="flex h-[27px]">
        <div class="flex-1"></div>
        <button
          class="border-l hover:bg-green-300 hover:text-layer-1 border-outline py-1 px-2 cursor-pointer"
          onclick={() => showPotentialRow()}
          aria-label="remove"
        >
          <span class="block i-[tabler--circle-plus]"></span>
        </button>
      </div>
    </div>
  {/if}
</div>
