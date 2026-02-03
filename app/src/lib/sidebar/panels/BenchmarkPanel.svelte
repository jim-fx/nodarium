<script lang="ts" module>
  let result:
    | { stdev: number; avg: number; duration: number; samples: number[] }
    | undefined = $state();
</script>

<script lang="ts">
  import { humanizeDuration } from '$lib/helpers';
  import { localState } from '$lib/helpers/localState.svelte';
  import Monitor from '$lib/performance/Monitor.svelte';
  import { InputNumber } from '@nodarium/ui';
  import { writable } from 'svelte/store';

  function calculateStandardDeviation(array: number[]) {
    const n = array.length;
    const mean = array.reduce((a, b) => a + b) / n;
    return Math.sqrt(
      array.map((x) => Math.pow(x - mean, 2)).reduce((a, b) => a + b) / n
    );
  }
  type Props = {
    run: () => Promise<unknown>;
  };

  const { run }: Props = $props();

  let isRunning = $state(false);
  let amount = localState<number>('nodes.benchmark.samples', 500);
  let samples = $state(0);
  let warmUp = writable(0);
  let warmUpAmount = 10;
  let status = '';

  const copyContent = async (text?: string | number) => {
    if (!text) return;
    if (typeof text !== 'string') {
      text = (Math.floor(text * 100) / 100).toString();
    }
    try {
      await navigator.clipboard.writeText(text);
    } catch (err) {
      console.error('Failed to copy: ', err);
    }
  };

  async function benchmark() {
    if (isRunning) return;
    isRunning = true;
    result = undefined;
    samples = 0;
    $warmUp = 0;

    await new Promise((r) => setTimeout(r, 50));

    // warm up
    for (let i = 0; i < warmUpAmount; i++) {
      await run();
      $warmUp = i + 1;
    }

    let a = performance.now();
    let results = [];

    // perform run
    for (let i = 0; i < amount.value; i++) {
      const a = performance.now();
      await run();
      samples = i;
      const b = performance.now();
      await new Promise((r) => setTimeout(r, 20));
      results.push(b - a);
    }
    result = {
      stdev: calculateStandardDeviation(results),
      samples: results,
      duration: performance.now() - a,
      avg: results.reduce((a, b) => a + b) / results.length
    };
  }
</script>

{status}

<div class="wrapper" class:running={isRunning}>
  {#if result}
    <h3>Finished ({humanizeDuration(result.duration)})</h3>
    <div class="monitor-wrapper">
      <Monitor points={result.samples} />
    </div>
    <label for="bench-avg">Average </label>
    <button
      id="bench-avg"
      onkeydown={(ev) => ev.key === 'Enter' && copyContent(result?.avg)}
      onclick={() => copyContent(result?.avg)}
    >
      {Math.floor(result.avg * 100) / 100}
    </button>
    <i
      role="button"
      tabindex="0"
      onkeydown={(ev) => ev.key === 'Enter' && copyContent(result?.avg)}
      onclick={() => copyContent(result?.avg)}
    >(click to copy)</i>
    <label for="bench-stdev">Standard Deviation σ</label>
    <button id="bench-stdev" onclick={() => copyContent(result?.stdev)}>
      {Math.floor(result.stdev * 100) / 100}
    </button>
    <i
      role="button"
      tabindex="0"
      onkeydown={(ev) => ev.key === 'Enter' && copyContent(result?.avg)}
      onclick={() => copyContent(result?.stdev + '')}
    >(click to copy)</i>
    <div>
      <button onclick={() => (isRunning = false)}>reset</button>
    </div>
  {:else if isRunning}
    <p>WarmUp ({$warmUp}/{warmUpAmount})</p>
    <progress value={$warmUp} max={warmUpAmount}>
      {Math.floor(($warmUp / warmUpAmount) * 100)}%
    </progress>
    <p>Progress ({samples}/{amount.value})</p>
    <progress value={samples} max={amount.value}>
      {Math.floor((samples / amount.value) * 100)}%
    </progress>
  {:else}
    <label for="bench-samples">Samples</label>
    <InputNumber id="bench-sample" bind:value={amount.value} max={1000} step={1} />
    <button onclick={benchmark} disabled={isRunning}>start</button>
  {/if}
</div>

<style>
  .wrapper {
    padding: 1em;
    display: flex;
    flex-direction: column;
    gap: 1em;
  }
  .monitor-wrapper {
    border: solid thin var(--color-outline);
    border-bottom: none;
  }
  i {
    opacity: 0.5;
    font-size: 0.8em;
  }
</style>
