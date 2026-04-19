<script lang="ts">
  import { fade } from 'svelte/transition';
  import type { Choice } from '../types.js';

  interface Props {
    text: string;
    avatarX: number;
    avatarY: number;
    choices?: Choice[];
    showNext?: boolean;
    stepIndex?: number;
    totalSteps?: number;
    onNext?: () => void;
    onClose?: () => void;
    onChoose?: (choice: Choice) => void;
  }

  let {
    text,
    avatarX,
    avatarY,
    choices = [],
    showNext = false,
    stepIndex = -1,
    totalSteps = 0,
    onNext,
    onClose,
    onChoose
  }: Props = $props();

  const showProgress = $derived(stepIndex >= 0 && totalSteps > 0);

  const BUBBLE_WIDTH = 268;
  const AVATAR_SIZE = 80;
  const GAP = 10;

  const isAvatarNearTop = $derived(avatarY < BUBBLE_WIDTH + GAP + 8);

  const left = $derived(Math.max(8, Math.min(avatarX, window.innerWidth - BUBBLE_WIDTH - 8)));
  const bottom = $derived(isAvatarNearTop ? null : `${window.innerHeight - avatarY + GAP}px`);
  const top = $derived(isAvatarNearTop ? `${avatarY + AVATAR_SIZE + GAP}px` : null);

  // Typewriter
  let displayed = $state('');
  const finished = $derived(displayed.length === text.length);
  let typeTimer: ReturnType<typeof setTimeout> | null = null;

  function renderMarkdown(raw: string): string {
    return raw
      .replaceAll(/^# (.+)$/gm, '<strong class="block text-sm font-bold mb-1">$1</strong>')
      .replaceAll(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replaceAll(/\*(.+?)\*/g, '<em>$1</em>')
      .replaceAll(
        /`(.+?)`/g,
        '<code class="text-[11px] rounded px-1 font-mono" style="background: var(--color-layer-3); color: var(--color-text);">$1</code>'
      )
      .replaceAll(/\*/, '')
      .replaceAll(/\_/, '')
      .replaceAll(/\n+/g, '<br>');
  }

  $effect(() => {
    // Track only `text` as a dependency.
    // Never read `displayed` inside the effect — += would add it as a dep
    // and cause an infinite loop. Use slice(0, i) for pure writes instead.
    const target = text;

    displayed = '';
    if (typeTimer) clearTimeout(typeTimer);

    let i = 0;
    function tick() {
      if (i < target.length) {
        displayed = target.slice(0, ++i);
        typeTimer = setTimeout(tick, 26);
      } else {
      }
    }
    // Defer first tick so no reads happen during the synchronous effect body
    typeTimer = setTimeout(tick, 0);

    return () => {
      if (typeTimer) clearTimeout(typeTimer);
    };
  });
</script>

<div
  class="fixed p-2 z-99999 pointer-events-auto rounded-md border"
  style:width="{BUBBLE_WIDTH}px"
  style:left="{left}px"
  style:bottom={bottom}
  style:top={top}
  style:background="var(--color-layer-0)"
  style:border-color="var(--color-outline)"
>
  {#if isAvatarNearTop}
    <!-- Tail pointing up toward avatar -->
    <div
      class="absolute -top-2 h-3.5 w-3.5 rotate-45 border-t border-l"
      style:left="{Math.min(Math.max(avatarX - left + AVATAR_SIZE / 2 - 25, 12), BUBBLE_WIDTH - 28)}px"
      style:background="var(--color-layer-0)"
      style:border-color="var(--color-outline)"
    >
    </div>
  {:else}
    <!-- Tail pointing down toward avatar -->
    <div
      class="absolute -bottom-2 h-3.5 w-3.5 rotate-45 border-b border-r"
      style:left="{Math.min(Math.max(avatarX - left + AVATAR_SIZE / 2 - 25, 12), BUBBLE_WIDTH - 28)}px"
      style:background="var(--color-layer-0)"
      style:border-color="var(--color-outline)"
    >
    </div>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="mb-2 min-h-[1.4em] text-sm leading-relaxed" style="color: var(--color-text)">
    {@html renderMarkdown(displayed)}
  </div>

  {#if choices.length > 0}
    <div class="flex flex-col gap-1.5">
      {#each choices as choice, i}
        {#if finished}
          <button
            in:fade={{ duration: 200, delay: i * 250 }}
            class="rounded-lg px-3 py-1.5 text-left text-sm font-medium transition-colors cursor-pointer"
            style:background="var(--color-layer-1)"
            style:border-color="var(--color-outline)"
            style:color="var(--color-text)"
            onclick={() => onChoose?.(choice)}
          >
            {choice.label}
          </button>
        {/if}
      {/each}
    </div>
  {/if}

  <div class="flex items-center justify-between gap-2 mt-2">
    <button
      class="text-xs transition-colors cursor-pointer"
      style="color: var(--color-outline)"
      onclick={onClose}
    >
      ✕ close
    </button>
    <div class="flex items-center gap-2">
      {#if showProgress}
        <span class="text-xs tabular-nums" style="color: var(--color-outline)">
          {stepIndex + 1} / {totalSteps}
        </span>
      {/if}
      {#if showNext && finished}
        <button
          class="rounded-lg px-3 py-1 text-xs font-semibold cursor-pointer transition-colors"
          style:background="var(--color-outline)"
          style:color="var(--color-layer-0)"
          onclick={onNext}
        >
          Next →
        </button>
      {/if}
    </div>
  </div>
</div>
