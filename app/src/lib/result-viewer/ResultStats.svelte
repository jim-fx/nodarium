<script lang="ts">
  import { humanizeDuration, humanizeNumber } from '#lib/helpers/index.ts';

  type Props = {
    vertices: number;
    faces: number;
    /** how long the build took */
    ms?: number;
    /** where it sits, it has no position of its own */
    class?: string;
  };

  const { vertices, faces, ms, class: className }: Props = $props();
</script>

<div
  class={['pointer-events-none z-2 flex flex-col text-[12px] opacity-70', className]}
  role="status"
>
  <span>{humanizeNumber(vertices)} vertices</span>
  <span>{humanizeNumber(faces)} faces</span>
  {#if ms !== undefined}
    <span>{humanizeDuration(ms)}</span>
  {/if}
</div>
