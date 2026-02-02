<script lang="ts">
  import { humanizeDuration, humanizeNumber } from '$lib/helpers';
  import { localState } from '$lib/helpers/localState.svelte';
  import type { PerformanceData, PerformanceStore } from '@nodarium/utils';
  import SmallGraph from './SmallGraph.svelte';

  const { store, fps }: { store: PerformanceStore; fps: number[] } = $props();

  const open = localState('node.performance.small.open', {
    runtime: false,
    fps: false
  });

  const vertices = $derived($store?.at(-1)?.['total-vertices']?.[0] || 0);
  const faces = $derived($store?.at(-1)?.['total-faces']?.[0] || 0);
  const runtime = $derived($store?.at(-1)?.['runtime']?.[0] || 0);

  function getPoints(data: PerformanceData, key: string) {
    return data?.map((run) => run[key]?.[0] || 0) || [];
  }
</script>

<div class="wrapper">
  <table>
    <tbody>
      <tr
        style="cursor: pointer"
        onclick={() => (open.value.runtime = !open.value.runtime)}
      >
        <td>{open.value.runtime ? '-' : '+'} runtime</td>
        <td>{humanizeDuration(runtime || 1000)}</td>
      </tr>
      {#if open.value.runtime}
        <tr>
          <td colspan="2">
            <SmallGraph points={getPoints($store, 'runtime')} />
          </td>
        </tr>
      {/if}

      <tr
        style="cursor: pointer"
        onclick={() => (open.value.fps = !open.value.fps)}
      >
        <td>{open.value.fps ? '-' : '+'} fps</td>
        <td>
          {Math.floor(fps[fps.length - 1])}fps
        </td>
      </tr>
      {#if open.value.fps}
        <tr>
          <td colspan="2">
            <SmallGraph points={fps} />
          </td>
        </tr>
      {/if}

      <tr>
        <td>vertices</td>
        <td>{humanizeNumber(vertices || 0)}</td>
      </tr>

      <tr>
        <td>faces</td>
        <td>{humanizeNumber(faces || 0)}</td>
      </tr>
    </tbody>
  </table>
</div>

<style>
  table {
    position: absolute;
    top: 10px;
    left: 10px;
    z-index: 2;
    background: var(--layer-0);
    border: solid thin var(--outline);
    border-collapse: collapse;
  }
  td {
    padding: 4px;
    padding-inline: 8px;
    font-size: 0.8em;
    border: solid thin var(--outline);
  }
</style>
