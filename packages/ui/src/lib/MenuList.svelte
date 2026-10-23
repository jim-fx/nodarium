<script lang="ts">
  import type { MenuAction, MenuEntry } from './menu';
  import Self from './MenuList.svelte';

  interface Props {
    items: MenuEntry[];
    label?: string;
    /** focus the first item when the menu opens */
    autofocus?: boolean;
    class?: string;
    style?: string;
    /** called after an item was selected or the menu was dismissed */
    onclose?: () => void;
    /** called when a submenu should close and give focus back to its parent */
    onback?: () => void;
    /** called with ArrowLeft/ArrowRight when there is no submenu to move into */
    onnavigate?: (direction: -1 | 1) => void;
  }

  let {
    items,
    label,
    autofocus = false,
    class: _class = '',
    style,
    onclose,
    onback,
    onnavigate
  }: Props = $props();

  let buttons = $state<HTMLButtonElement[]>([]);
  let openIndex = $state<number | null>(null);
  let focusSubmenu = $state(false);

  const enabledIndices = $derived(
    items.flatMap((entry, i) => entry !== 'separator' && !entry.disabled ? [i] : [])
  );

  $effect(() => {
    if (autofocus && enabledIndices.length) {
      buttons[enabledIndices[0]]?.focus();
    }
  });

  function move(current: number, direction: -1 | 1) {
    const position = enabledIndices.indexOf(current);
    const next = enabledIndices[
      (position + direction + enabledIndices.length) % enabledIndices.length
    ];
    buttons[next]?.focus();
  }

  function select(item: MenuAction, index: number, viaKeyboard: boolean) {
    if (item.disabled) return;
    if (item.items) {
      openIndex = index;
      focusSubmenu = viaKeyboard;
      return;
    }
    item.onselect?.();
    onclose?.();
  }

  function handleKeydown(event: KeyboardEvent, item: MenuAction, index: number) {
    switch (event.key) {
      case 'ArrowDown':
        move(index, 1);
        break;
      case 'ArrowUp':
        move(index, -1);
        break;
      case 'Home':
        buttons[enabledIndices[0]]?.focus();
        break;
      case 'End':
        buttons[enabledIndices[enabledIndices.length - 1]]?.focus();
        break;
      case 'ArrowRight':
        if (item.items) select(item, index, true);
        else onnavigate?.(1);
        break;
      case 'ArrowLeft':
        if (onback) onback();
        else onnavigate?.(-1);
        break;
      case 'Escape':
        if (onback) onback();
        else onclose?.();
        break;
      case 'Enter':
      case ' ':
        select(item, index, true);
        break;
      case 'Tab':
        onclose?.();
        return;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  }

  function handleMouseEnter(item: MenuAction, index: number) {
    buttons[index]?.focus({ preventScroll: true });
    openIndex = item.items && !item.disabled ? index : null;
    focusSubmenu = false;
  }
</script>

<ul
  role="menu"
  aria-label={label}
  class="z-50 m-0 min-w-60 list-none border border-outline bg-layer-1 p-0 {_class}"
  {style}
>
  {#each items as item, i (i)}
    {#if item === 'separator'}
      <li role="separator" class="border-t border-outline"></li>
    {:else}
      <li role="none" class="relative">
        <button
          bind:this={buttons[i]}
          type="button"
          tabindex="-1"
          role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
          aria-checked={item.checked}
          aria-haspopup={item.items ? 'menu' : undefined}
          aria-expanded={item.items ? openIndex === i : undefined}
          disabled={item.disabled}
          class="
            flex w-full cursor-pointer items-center gap-3 border-none bg-transparent px-3.5 text-left text-sm
            font-(--font-family) outline-none hover:bg-layer-2 focus:bg-layer-2
            disabled:cursor-default disabled:opacity-40
            {item.image !== undefined ? 'h-11' : 'h-8.5'}
            {openIndex === i ? 'bg-layer-2' : ''}
          "
          onclick={() => select(item, i, false)}
          onmouseenter={() => handleMouseEnter(item, i)}
          onkeydown={(e) => handleKeydown(e, item, i)}
        >
          {#if item.checked !== undefined}
            <span class="block h-4 w-4 shrink-0 {item.checked ? 'i-[tabler--check]' : ''}"></span>
          {/if}
          {#if item.icon}
            <span class="block h-4 w-4 shrink-0 {item.icon}"></span>
          {/if}
          {#if item.image !== undefined}
            <span class="block h-7 w-7 shrink-0 border border-outline bg-layer-0">
              {#if item.image}
                <img src={item.image} alt="" class="block h-full w-full object-contain" />
              {/if}
            </span>
          {/if}
          <span class="grow truncate">{item.label}</span>
          {#if item.detail ?? item.shortcut}
            <span class="whitespace-nowrap text-xs opacity-50">{item.detail ?? item.shortcut}</span>
          {/if}
          {#if item.items}
            <span class="block h-3.5 w-3.5 shrink-0 opacity-50 i-[tabler--chevron-right]"></span>
          {/if}
        </button>
        {#if item.items && openIndex === i}
          <Self
            items={item.items}
            label={item.label}
            autofocus={focusSubmenu}
            class="absolute -top-px left-full"
            {onclose}
            {onnavigate}
            onback={() => {
              openIndex = null;
              buttons[i]?.focus();
            }}
          />
        {/if}
      </li>
    {/if}
  {/each}
</ul>
