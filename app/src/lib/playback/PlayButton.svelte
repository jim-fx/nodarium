<script lang="ts">
  import { Button } from '@nodarium/ui';

  type Props = {
    status: 'idle' | 'playing' | 'paused';
    onclick: () => void;
    /** The round button over the viewer, for a finger */
    round?: boolean;
  };

  const { status, onclick, round = false }: Props = $props();

  const playing = $derived(status === 'playing');
  const label = $derived(
    playing ? 'Pause growth' : status === 'paused' ? 'Resume growth' : 'Play growth'
  );
</script>

{#if round}
  <button
    type="button"
    aria-label={label}
    title={label}
    class="absolute bottom-4 left-1/2 z-2 flex h-12 w-12 -translate-x-1/2 cursor-pointer touch-manipulation items-center justify-center rounded-full border border-transparent bg-selected p-0 text-white hover:opacity-88"
    {onclick}
  >
    <span
      class="block h-5 w-5 {playing ? 'i-[tabler--player-pause-filled]' : 'i-[tabler--player-play-filled]'}"
    ></span>
  </button>
{:else}
  <div class="p-4">
    <Button {onclick}>{label}</Button>
  </div>
{/if}
