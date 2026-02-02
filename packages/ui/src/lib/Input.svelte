<script lang="ts">
  import type { NodeInput } from '@nodarium/types';

  import { Checkbox, Float, Select, Vec3 } from './index.js';

  interface Props {
    input: NodeInput;
    value: unknown;
    id?: string;
  }

  let { input, value = $bindable(), id }: Props = $props();
</script>

{#if input.type === 'float'}
  <Float
    bind:value={value as number}
    min={input?.min}
    max={input?.max}
    step={input?.step}
  />
{:else if input.type === 'integer'}
  <Float bind:value={value as number} min={input?.min} max={input?.max} />
{:else if input.type === 'boolean'}
  <Checkbox bind:value={value as boolean} {id} />
{:else if input.type === 'select'}
  <Select bind:value={value as number} options={input.options} {id} />
{:else if input.type === 'vec3'}
  <Vec3 bind:value={value as [number, number, number]} {id} />
{/if}
