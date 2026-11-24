<script lang="ts">
  type Props = {
    status: 'idle' | 'playing' | 'paused';
    onclick: () => void;
    class?: string;
  };

  const { status, onclick, class: className }: Props = $props();

  const playing = $derived(status === 'playing');
  const label = $derived(
    playing ? 'Pause growth' : status === 'paused' ? 'Resume growth' : 'Play growth'
  );
</script>

<button
  type="button"
  aria-label={label}
  title={label}
  class={[
    'z-2 flex h-12 w-12 cursor-pointer touch-manipulation items-center justify-center rounded-full border border-transparent bg-selected p-0 text-white hover:opacity-88',
    className
  ]}
  {onclick}
>
  <span
    class="block h-5 w-5 {playing ? 'i-[tabler--player-pause-filled]' : 'i-[tabler--player-play-filled]'}"
  ></span>
</button>
