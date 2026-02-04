<script lang="ts">
  import { localState } from '$lib/helpers/localState.svelte';
  import type { NodeInput } from '@nodarium/types';
  import Input from '@nodarium/ui';
  import { onMount } from 'svelte';
  import NestedSettings from './NestedSettings.svelte';

  type Button = { type: 'button'; label?: string };

  type InputType = NodeInput | Button;

  type SettingsNode = InputType | SettingsGroup;

  interface SettingsGroup {
    title?: string;
    [key: string]: unknown;
  }

  type SettingsType = Record<string, SettingsNode>;

  type SettingsValue = Record<
    string,
    Record<string, unknown> | string | number | boolean | number[]
  >;

  type Props = {
    id: string;
    key?: string;
    value: SettingsValue;
    type: SettingsType;
    depth?: number;
  };

  // Local persistent state for <details> sections
  const openSections = localState<Record<string, boolean>>('open-details', {});

  let { id, key = '', value = $bindable(), type, depth = 0 }: Props = $props();

  function isNodeInput(v: SettingsNode | undefined): v is InputType {
    return !!v && typeof v === 'object' && 'type' in v;
  }

  function getDefaultValue(): NodeInput['value'] | undefined {
    if (key === '' || key === 'title') return;

    const node = type[key] as SettingsNode;
    const inputValue = value[key];

    if (!isNodeInput(node)) return;

    // select input: use index into options
    if ('options' in node && Array.isArray(node.options)) {
      if (typeof inputValue === 'string') {
        return node.options.indexOf(inputValue);
      }
      return 0;
    }

    if (Array.isArray(inputValue) && node.type === 'vec3') {
      return inputValue;
    }

    // If the component is supplied with a default value use that
    if (inputValue !== undefined && typeof inputValue !== 'object') {
      return inputValue;
    }

    if ('value' in node) {
      const nodeValue = node.value;
      if (nodeValue !== null && nodeValue !== undefined) {
        return nodeValue;
      }
    }

    switch (node.type) {
      case 'boolean':
        return 0;
      case 'float':
        return 0.5;
      case 'integer':
      case 'select':
        return 0;
      default:
        return 0;
    }
  }

  let internalValue = $state(getDefaultValue());

  let open = $state(false);

  // Sync internalValue back into `value`
  $effect(() => {
    if (key === '' || internalValue === undefined) return;

    const node = type[key];

    if (
      isNodeInput(node)
      && 'options' in node
      && Array.isArray(node.options)
      && typeof internalValue === 'number'
    ) {
      value[key] = node?.options?.[internalValue];
    } else if (internalValue !== undefined) {
      value[key] = internalValue;
    }
  });

  function handleClick() {
    const callback = value[key] as unknown as () => void;
    callback();
  }

  onMount(() => {
    open = openSections.value[id];

    // Persist <details> open/closed state for groups
    if (depth > 0 && !isNodeInput(type[key!])) {
      $effect(() => {
        if (open !== undefined) {
          openSections.value[id] = open;
        }
      });
    }
  });
</script>

{#if key && isNodeInput(type?.[key])}
  {@const inputType = type[key]}
  <div class="input input-{inputType.type}" class:first-level={depth === 1}>
    {#if inputType.type === 'button'}
      <button onclick={handleClick}>
        {inputType.label || key}
      </button>
    {:else}
      {#if inputType.label !== ''}
        <label for={id}>{inputType.label || key}</label>
      {/if}
      <Input {id} input={inputType} bind:value={internalValue} />
    {/if}
  </div>
{:else if depth === 0}
  {#each Object.keys(type ?? {}).filter((k) => k !== 'title') as childKey (childKey)}
    <NestedSettings
      id={`${id}.${childKey}`}
      key={childKey}
      bind:value
      {type}
      depth={depth + 1}
    />
  {/each}
  <hr />
{:else if key && type?.[key]}
  {#if depth > 0}
    <hr />
  {/if}
  <details bind:open>
    <summary><p>{(type[key] as SettingsGroup).title || key}</p></summary>
    <div class="content">
      {#each Object.keys(type[key] as SettingsGroup).filter((k) => k !== 'title') as childKey (childKey)}
        <NestedSettings
          id={`${id}.${childKey}`}
          key={childKey}
          bind:value={value[key] as SettingsValue}
          type={type[key] as unknown as SettingsType}
          depth={depth + 1}
        />
      {/each}
    </div>
  </details>
{/if}

<style>
  summary {
    cursor: pointer;
    user-select: none;
    margin-bottom: 1em;
  }

  summary > p {
    display: inline;
    padding-left: 6px;
  }

  details {
    padding: 1em;
    padding-bottom: 0;
    padding-left: 21px;
  }

  .input {
    margin-top: 15px;
    margin-bottom: 15px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-left: 20px;
  }

  .input-boolean {
    display: flex;
    flex-direction: row;
    align-items: center;
  }

  .input-boolean > label {
    order: 2;
  }

  .first-level.input {
    padding-left: 1em;
    padding-right: 1em;
    padding-bottom: 1px;
    gap: 3px;
  }

  .first-level.input-boolean {
    gap: 10px;
  }

  button {
    cursor: pointer;
  }

  hr {
    margin: 0;
    left: 0;
    right: 0;
    border: none;
    border-bottom: solid thin var(--color-outline);
  }
</style>
