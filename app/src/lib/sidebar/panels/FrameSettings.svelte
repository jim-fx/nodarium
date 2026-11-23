<script lang="ts">
  import type { GraphManager, GraphState } from '@nodarium/graph-interface';
  import { FRAME_COLORS } from '@nodarium/types';

  type Props = { manager: GraphManager; graphState?: GraphState };

  const { manager, graphState }: Props = $props();

  const frame = $derived(graphState?.activeFrame);
</script>

{#if frame}
  <div class="bg-layer-2 flex items-center h-[70px] border-b-1 border-b-outline pl-4">
    <h3>Frame</h3>
  </div>
  {#key frame.id}
    <div class="p-4 flex flex-col gap-[0.4em]">
      <label for="frame-label" class="text-[0.8em] opacity-70">Title</label>
      <input
        id="frame-label"
        type="text"
        class="bg-layer-1 border border-outline rounded-[4px] text-text text-[0.9em] px-[0.6em] py-[0.4em] focus:outline focus:outline-active"
        value={frame.label}
        onchange={(e) => {
          const label = e.currentTarget.value.trim();
          if (label) manager.renameFrame(frame.id, label);
        }}
      />
      <span id="frame-color" class="text-[0.8em] opacity-70">Colour</span>
      <div class="flex gap-2" role="radiogroup" aria-labelledby="frame-color">
        {#each FRAME_COLORS as color (color)}
          {@const checked = (frame.color ?? 'gray') === color}
          <button
            type="button"
            role="radio"
            aria-checked={checked}
            aria-label={color}
            title={color}
            class="
              w-6 h-6 rounded-sm cursor-pointer outline-offset-2 {checked
              ? 'outline-2 outline-active'
              : 'outline-1 outline-outline'}
            "
            style:background="var(--color-frame-{color})"
            onclick={() => manager.setFrameColor(frame.id, color)}
          >
          </button>
        {/each}
      </div>
    </div>
  {/key}
{/if}
