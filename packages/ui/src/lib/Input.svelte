<script lang="ts">
  import type { NodeInput } from '@nodarium/types';

  import {
    InputCheckbox,
    InputColor,
    InputNumber,
    InputSelect,
    InputShape,
    InputVec3
  } from './index';

  interface Props {
    input: NodeInput;
    value: unknown;
    id?: string;
  }

  let { input, value = $bindable(), id }: Props = $props();
</script>

{#if input.type === 'float'}
  <InputNumber
    bind:value={value as number}
    min={input?.min}
    max={input?.max}
    step={input?.step}
  />
{:else if input.type === 'shape'}
  <InputShape bind:value={value as number[]} />
{:else if input.type === 'color'}
  <InputColor bind:value={value as [number, number, number]} />
{:else if input.type === 'integer'}
  <InputNumber
    bind:value={value as number}
    min={input?.min}
    max={input?.max}
    step={1}
  />
{:else if input.type === 'boolean'}
  <InputCheckbox bind:value={value as boolean} {id} />
{:else if input.type === 'select'}
  <InputSelect bind:value={value as number} options={input.options} {id} />
{:else if input.type === 'vec3'}
  <InputVec3 bind:value={value as [number, number, number]} {id} />
{/if}
