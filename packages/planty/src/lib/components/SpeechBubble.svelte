<script lang="ts">
  import { prefersReducedMotion } from 'svelte/motion';
  import { innerHeight, innerWidth } from 'svelte/reactivity/window';
  import { fade } from 'svelte/transition';
  import type { Choice } from '../types.js';

  interface Props {
    text: string;
    title?: string;
    avatarX: number;
    avatarY: number;
    choices?: Choice[];
    showNext?: boolean;
    stepIndex?: number;
    totalSteps?: number;
    onNext?: () => void;
    onClose?: () => void;
    onChoose?: (choice: Choice) => void;
    onTypingDone?: () => void;
  }

  let {
    text,
    title,
    avatarX,
    avatarY,
    choices = [],
    showNext = false,
    stepIndex = -1,
    totalSteps = 0,
    onNext,
    onClose,
    onChoose,
    onTypingDone
  }: Props = $props();

  const showProgress = $derived(stepIndex >= 0 && totalSteps > 1);

  const BUBBLE_WIDTH = 268;
  const AVATAR_SIZE = 80;
  const GAP = 10;
  const TYPE_MS = 26;

  const isAvatarNearTop = $derived(avatarY < BUBBLE_WIDTH + GAP + 8);

  const left = $derived(
    Math.max(8, Math.min(avatarX, (innerWidth.current ?? 0) - BUBBLE_WIDTH - 8))
  );
  const bottom = $derived(
    isAvatarNearTop ? null : `${(innerHeight.current ?? 0) - avatarY + GAP}px`
  );
  const top = $derived(isAvatarNearTop ? `${avatarY + AVATAR_SIZE + GAP}px` : null);
  const tailLeft = $derived(
    Math.min(Math.max(avatarX - left + AVATAR_SIZE / 2 - 25, 12), BUBBLE_WIDTH - 28)
  );

  function escapeHtml(raw: string): string {
    return raw.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
  }

  function renderMarkdown(raw: string): string {
    // pull out code spans first so their content is not parsed as emphasis
    const code: string[] = [];
    return escapeHtml(raw)
      .replaceAll(/`(.+?)`/g, (_, c: string) => `\uE000${code.push(c) - 1}\uE000`)
      .replaceAll(/^# (.+)$/gm, '<strong class="block text-sm font-bold mb-1">$1</strong>')
      .replaceAll(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replaceAll(/\*(.+?)\*/g, '<em>$1</em>')
      .replaceAll(/\n+/g, '<br>')
      .replaceAll(
        /\uE000(\d+)\uE000/g,
        (_, i: string) =>
          `<code class="text-[11px] rounded px-1 font-mono" style="background: var(--color-layer-3); color: var(--color-text);">${
            code[Number(i)]
          }</code>`
      );
  }

  function toPlainText(raw: string): string {
    return raw
      .replaceAll(/^# /gm, '')
      .replaceAll(/\*\*(.+?)\*\*/g, '$1')
      .replaceAll(/\*(.+?)\*/g, '$1')
      .replaceAll('`', '');
  }

  /** Offsets into `html` after each visible character, tags and entities are never split */
  function revealPoints(html: string): number[] {
    const points: number[] = [];
    let i = 0;
    while (i < html.length) {
      if (html[i] === '<') {
        const end = html.indexOf('>', i);
        i = end === -1 ? html.length : end + 1;
        continue;
      }
      if (html[i] === '&') {
        const end = html.indexOf(';', i);
        i = end === -1 ? html.length : end + 1;
      } else {
        i += html.codePointAt(i)! > 0xffff ? 2 : 1;
      }
      points.push(i);
    }
    // include trailing closing tags in the last step
    if (points.length) points[points.length - 1] = html.length;
    return points;
  }

  // Typewriter, unclosed tags of a partial slice are closed by the HTML parser
  const html = $derived(renderMarkdown(text));
  const points = $derived(revealPoints(html));
  let shown = $state(0);
  const finished = $derived(shown >= points.length);
  const displayed = $derived(finished ? html : html.slice(0, points[shown - 1] ?? 0));

  $effect(() => {
    const total = points.length;
    if (prefersReducedMotion.current) {
      shown = total;
      return;
    }
    shown = 0;
    const id = setInterval(() => {
      if (shown >= total) clearInterval(id);
      else shown++;
    }, TYPE_MS);
    return () => clearInterval(id);
  });

  $effect(() => {
    if (finished) onTypingDone?.();
  });

  function skip() {
    shown = points.length;
  }

  function isEditable(target: EventTarget | null) {
    return (
      target instanceof HTMLElement
      && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))
    );
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey || e.altKey || isEditable(e.target)) return;
    if (e.key === 'Escape') {
      onClose?.();
    } else if (e.key === 'Enter' && !(e.target instanceof HTMLButtonElement)) {
      if (!finished) skip();
      else if (showNext) onNext?.();
      else return;
      e.preventDefault();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="pointer-events-auto fixed z-99999 rounded-md border p-2"
  role="dialog"
  aria-label={title}
  style:width="{BUBBLE_WIDTH}px"
  style:left="{left}px"
  style:bottom
  style:top
  style:background="var(--color-layer-0)"
  style:border-color="var(--color-outline)"
>
  {#if isAvatarNearTop}
    <!-- Tail pointing up toward avatar -->
    <div
      class="absolute -top-2 h-3.5 w-3.5 rotate-45 border-t border-l"
      style:left="{tailLeft}px"
      style:background="var(--color-layer-0)"
      style:border-color="var(--color-outline)"
    >
    </div>
  {:else}
    <!-- Tail pointing down toward avatar -->
    <div
      class="absolute -bottom-2 h-3.5 w-3.5 rotate-45 border-r border-b"
      style:left="{tailLeft}px"
      style:background="var(--color-layer-0)"
      style:border-color="var(--color-outline)"
    >
    </div>
  {/if}

  <p class="sr-only" aria-live="polite">{toPlainText(text)}</p>

  <!-- Enter skips the typewriter as well, see handleKeydown -->
  <div
    class="mb-2 min-h-[1.4em] text-sm leading-relaxed"
    class:cursor-pointer={!finished}
    style="color: var(--color-text)"
    aria-hidden="true"
    onclick={skip}
  >
    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
    {@html displayed}
  </div>

  {#if choices.length > 0}
    <div class="flex flex-col gap-1.5">
      {#each choices as choice, i (choice.label)}
        {#if finished}
          <button
            in:fade={prefersReducedMotion.current
            ? { duration: 0 }
            : { duration: 200, delay: i * 250 }}
            class="cursor-pointer rounded-lg px-3 py-1.5 text-left text-sm font-medium transition-colors"
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

  <div class="mt-2 flex items-center justify-between gap-2">
    <button
      class="cursor-pointer text-xs transition-colors"
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
          class="cursor-pointer rounded-lg px-3 py-1 text-xs font-semibold transition-colors"
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
