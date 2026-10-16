<script lang="ts">
  import type { Menu } from './menu';
  import MenuList from './MenuList.svelte';

  interface Props {
    menus: Menu[];
    label?: string;
  }

  let { menus, label = 'Menu' }: Props = $props();

  let root = $state<HTMLElement>();
  let triggers = $state<HTMLButtonElement[]>([]);
  let openIndex = $state<number | null>(null);
  let focusItems = $state(false);

  function open(index: number, viaKeyboard: boolean) {
    openIndex = index;
    focusItems = viaKeyboard;
  }

  function close(focusTrigger = true) {
    const index = openIndex;
    openIndex = null;
    if (focusTrigger && index !== null) triggers[index]?.focus();
  }

  function navigate(direction: -1 | 1) {
    if (openIndex === null) return;
    open((openIndex + direction + menus.length) % menus.length, true);
  }

  function handleTriggerKeydown(event: KeyboardEvent, index: number) {
    if (event.key === 'ArrowDown' || event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      open(index, true);
    } else if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') {
      event.preventDefault();
      const next = (index + (event.key === 'ArrowRight' ? 1 : -1) + menus.length) % menus.length;
      triggers[next]?.focus();
    } else if (event.key === 'Escape') {
      close();
    }
  }

  function handleFocusOut(event: FocusEvent) {
    if (openIndex !== null && !root?.contains(event.relatedTarget as Node | null)) {
      close(false);
    }
  }

  function handleWindowPointerDown(event: PointerEvent) {
    if (openIndex !== null && !root?.contains(event.target as Node)) {
      close(false);
    }
  }
</script>

<svelte:window onpointerdown={handleWindowPointerDown} />

<div
  bind:this={root}
  role="menubar"
  aria-label={label}
  tabindex="-1"
  class="flex h-full items-stretch outline-none"
  onfocusout={handleFocusOut}
>
  {#each menus as menu, i (menu.label)}
    <div class="relative">
      <button
        bind:this={triggers[i]}
        type="button"
        aria-haspopup="menu"
        aria-expanded={openIndex === i}
        class="
          h-full cursor-pointer border-y-0 border-l-0 border-r border-outline px-3.5 text-sm
          font-(--font-family) outline-none hover:bg-layer-2 focus-visible:bg-layer-2
          {openIndex === i ? 'bg-layer-2' : 'bg-transparent'}
        "
        onclick={() => (openIndex === i ? close() : open(i, false))}
        onmouseenter={() => openIndex !== null && openIndex !== i && open(i, false)}
        onkeydown={(e) => handleTriggerKeydown(e, i)}
      >
        {menu.label}
      </button>
      {#if openIndex === i}
        <MenuList
          items={menu.items}
          label={menu.label}
          autofocus={focusItems}
          class="absolute -left-px top-full"
          onclose={() => close()}
          onnavigate={navigate}
        />
      {/if}
    </div>
  {/each}
</div>
