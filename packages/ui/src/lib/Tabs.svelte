<script lang="ts" generics="T extends string">
  import type { Snippet } from 'svelte';

  interface Props {
    tabs: readonly T[];
    value?: T;
    label?: string;
    class?: string;
    children?: Snippet<[T]>;
  }

  let {
    tabs,
    value = $bindable(tabs[0]),
    label = 'Tabs',
    class: _class = '',
    children
  }: Props = $props();

  const id = $props.id();
  let buttons = $state<HTMLButtonElement[]>([]);

  function select(index: number) {
    value = tabs[index];
    buttons[index]?.focus();
  }

  function handleKeydown(event: KeyboardEvent, index: number) {
    const last = tabs.length - 1;
    const next = {
      ArrowRight: index === last ? 0 : index + 1,
      ArrowLeft: index === 0 ? last : index - 1,
      Home: 0,
      End: last
    }[event.key];
    if (next === undefined) return;
    event.preventDefault();
    select(next);
  }
</script>

<div class={_class}>
  <div
    role="tablist"
    aria-label={label}
    class="flex rounded-[2px] outline-1 outline-outline bg-layer-1 overflow-hidden"
  >
    {#each tabs as tab, i (tab)}
      {@const active = tab === value}
      <button
        bind:this={buttons[i]}
        type="button"
        role="tab"
        id="{id}-tab-{i}"
        aria-selected={active}
        aria-controls="{id}-panel"
        tabindex={active ? 0 : -1}
        class="
          flex-1 cursor-pointer border-y-0 border-l-0 border-r border-outline last:border-r-0
          px-3.5 py-2 text-sm font-(--font-family) font-light text-text outline-none
          focus-visible:bg-layer-2
          {active ? 'bg-layer-2' : 'bg-transparent opacity-50 hover:opacity-100'}
        "
        onclick={() => (value = tab)}
        onkeydown={(e) => handleKeydown(e, i)}
      >
        {tab}
      </button>
    {/each}
  </div>
  {#if children && value !== undefined}
    <div
      role="tabpanel"
      id="{id}-panel"
      aria-labelledby="{id}-tab-{tabs.indexOf(value)}"
      class="pt-5"
    >
      {@render children(value)}
    </div>
  {/if}
</div>
