<script lang="ts">
  import Planty from '$lib/components/Planty.svelte';
  import PlantyAvatar, { type Mood } from '$lib/components/PlantyAvatar.svelte';
  import type { PlantyConfig } from '$lib/types.js';
  import { onMount } from 'svelte';
  import ThemeSelector from './ThemeSelector.svelte';

  let plantyConfig = $state<PlantyConfig | null>(null);
  let planty: ReturnType<typeof Planty> | undefined = $state();
  let started = $state(false);

  // Avatar preview state
  const moods: Mood[] = ['idle', 'talking', 'happy', 'thinking', 'moving'];
  let previewMood = $state<Mood>('idle');

  onMount(async () => {
    const res = await fetch('/demo-tutorial.json');
    plantyConfig = await res.json();
  });

  function startTour() {
    planty?.start();
    started = true;
  }
</script>

<svelte:head>
  <title>Planty — Demo</title>
</svelte:head>

<div
  class="grid min-h-screen grid-rows-[auto_1fr]"
  style="background-color: var(--color-layer-0); color: var(--color-text);"
>
  <!-- Header -->
  <header
    class="flex h-12 items-center gap-4 px-8 py-5"
    style="border-color: var(--color-outline);"
  >
    <h1 class="text-xl font-semibold">🌿 Planty</h1>
    <span
      class="rounded-full px-2 py-0.5 text-xs font-bold"
      style="background: var(--color-layer-3); color: var(--color-layer-0);"
    >demo</span>

    <ThemeSelector />

    <button
      class="ml-auto rounded-xl px-5 py-2 text-sm font-bold transition hover:scale-95 active:scale-95"
      style="background: var(--color-layer-3); color: var(--color-layer-0);"
      onclick={startTour}
      disabled={started || !plantyConfig}
    >
      {started ? 'Tour running…' : 'Start tutorial'}
    </button>
  </header>

  <!-- App layout -->
  <main class="grid grid-cols-[1fr_280px]">
    <!-- Graph canvas -->
    <div
      id="graph-canvas"
      class="relative flex min-h-125 items-center justify-center"
      style="background-color: var(--color-layer-1); background-image: radial-gradient(circle, var(--color-outline) 1px, transparent 1px); background-size: 24px 24px;"
    >
      <p class="text-center text-sm" style="color: var(--color-outline);">
        Node graph canvas<br />
        <span style="opacity: 0.6;">(click "Start tutorial" above)</span>
      </p>

      <!-- Avatar mood preview (bottom of canvas) -->
      <div class="absolute bottom-6 left-1/2 flex -translate-x-1/2 flex-col items-center gap-4">
        <!-- Static preview at fixed position inside the canvas -->
        <div class="relative h-20 w-12">
          <PlantyAvatar x={0} y={0} mood={previewMood} />
        </div>
        <div class="flex gap-2">
          {#each moods as m (m)}
            <button
              class="rounded-lg border px-3 py-1 text-xs transition"
              onclick={() => (previewMood = m)}
              style="border-color: {previewMood === m
  ? 'var(--color-selected)'
  : 'var(--color-outline)'}; color: {previewMood === m
  ? 'var(--color-selected)'
  : 'var(--color-text)'}; background: {previewMood === m
  ? 'var(--color-layer-2)'
  : 'transparent'};"
            >
              {m}
            </button>
          {/each}
        </div>
      </div>
    </div>

    <!-- Sidebar -->
    <aside
      id="sidebar"
      class="flex flex-col gap-3 p-5"
      style="border-color: var(--color-outline); background-color: var(--color-layer-0);"
    >
      <span
        class="text-xs font-semibold tracking-widest uppercase"
        style="color: var(--color-outline);"
      >Parameters</span>
      <div
        class="rounded-lg px-3 py-2 text-sm"
        style="background: var(--color-layer-1); color: var(--color-text);"
      >
        Branch length: 1.0
      </div>
      <div
        class="rounded-lg px-3 py-2 text-sm"
        style="background: var(--color-layer-1); color: var(--color-text);"
      >
        Segments: 8
      </div>
      <div
        class="rounded-lg px-3 py-2 text-sm"
        style="background: var(--color-layer-1); color: var(--color-text);"
      >
        Leaf density: 0.6
      </div>
      <span
        class="mt-2 text-xs font-semibold tracking-widest uppercase"
        style="color: var(--color-outline);"
      >Export</span>
      <div
        class="rounded-lg px-3 py-2 text-sm"
        style="background: var(--color-layer-1); color: var(--color-text);"
      >
        .obj / .glb
      </div>
    </aside>
  </main>
</div>

{#if plantyConfig}
  <Planty
    bind:this={planty}
    config={plantyConfig}
    onComplete={() => {
      started = false;
    }}
  />
{/if}
