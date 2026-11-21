<script lang="ts">
  type Props = {
    value: number;
    onchange: (t: number) => void;
    /** seconds from 0 to 1 while playing */
    duration?: number;
  };

  const { value, onchange, duration = 6 }: Props = $props();

  let playing = $state(false);

  $effect(() => {
    if (!playing) return;
    let frame = 0;
    let last = performance.now();
    const step = (now: number) => {
      const t = Math.min(1, value + (now - last) / 1000 / duration);
      last = now;
      onchange(t);
      if (t >= 1) playing = false;
      else frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  });

  function toggle() {
    if (!playing && value >= 1) onchange(0);
    playing = !playing;
  }

  function scrub(event: Event) {
    playing = false;
    onchange(Number((event.currentTarget as HTMLInputElement).value));
  }
</script>

<div class="flex min-w-0 items-center gap-3 text-xs">
  <button
    type="button"
    class="flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded border border-outline bg-layer-2 text-text hover:opacity-85"
    aria-label={playing ? 'Pause' : 'Play'}
    onclick={toggle}
  >
    <span
      class="block h-3.5 w-3.5 {playing ? 'i-[tabler--player-pause-filled]' : 'i-[tabler--player-play-filled]'}"
    ></span>
  </button>
  <div class="relative flex h-7 min-w-0 flex-1 items-center">
    <div class="absolute inset-x-0 h-1 rounded-full bg-layer-2"></div>
    <div class="absolute left-0 h-1 rounded-full bg-selected" style:width="{value * 100}%"></div>
    <input
      type="range"
      min="0"
      max="1"
      step="0.001"
      aria-label="Time"
      {value}
      oninput={scrub}
      class="peer absolute inset-0 m-0 h-full w-full cursor-pointer opacity-0"
    />
    <div
      class="pointer-events-none absolute h-3 w-3 -translate-x-1/2 rounded-full bg-text peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-selected"
      style:left="{value * 100}%"
    >
    </div>
  </div>
  <output class="w-9 shrink-0 text-right tabular-nums opacity-70">{value.toFixed(2)}</output>
</div>
