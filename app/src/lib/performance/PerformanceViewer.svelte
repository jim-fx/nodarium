<script lang="ts">
  import { humanizeNumber } from '$lib/helpers';
  import { Checkbox } from '@nodarium/ui';
  import type { PerformanceData } from '@nodarium/utils';
  import BarSplit from './BarSplit.svelte';
  import Monitor from './Monitor.svelte';

  const { data }: { data: PerformanceData } = $props();

  let activeType = $state('total');
  let showAverage = $state(true);

  function round(v: number) {
    if (v < 1) {
      return Math.floor(v * 100) / 100;
    }
    if (v < 10) {
      return Math.floor(v * 10) / 10;
    }
    return Math.floor(v);
  }

  function getTitle(t: string) {
    if (t.includes('/')) {
      return `Node ${t.split('/').slice(-1).join('/')}`;
    }

    return t
      .split('-')
      .map((v) => v[0].toUpperCase() + v.slice(1))
      .join(' ');
  }

  const viewerKeys = [
    'total-vertices',
    'total-faces',
    'update-geometries',
    'split-result'
  ];

  // --- Small helpers that query `data` directly ---
  function getAverage(key: string) {
    const vals = data
      .map((run) => run[key]?.[0])
      .filter((v) => v !== undefined) as number[];

    if (vals.length === 0) return 0;
    return vals.reduce((acc, v) => acc + v, 0) / vals.length;
  }

  function getLast(key: string) {
    return data.at(-1)?.[key]?.[0] || 0;
  }

  const averages = $derived.by(() => {
    const lr = data.at(-1);
    if (!lr) return {} as Record<string, number>;
    return Object.keys(lr).reduce((acc: Record<string, number>, key) => {
      acc[key] = getAverage(key);
      return acc;
    }, {});
  });

  const lasts = $derived.by(() => data.at(-1) || {});

  const totalPerformance = $derived.by(() => {
    const onlyLast = getLast('runtime')
      + getLast('update-geometries')
      + getLast('worker-transfer');
    const average = getAverage('runtime')
      + getAverage('update-geometries')
      + getAverage('worker-transfer');
    return { onlyLast, average };
  });

  const cacheRatio = $derived.by(() => {
    return {
      onlyLast: Math.floor(getLast('cache-hit') * 100),
      average: Math.floor(getAverage('cache-hit') * 100)
    };
  });

  const performanceData = $derived.by(() => {
    const source = showAverage ? averages : lasts;
    return Object.entries(source)
      .filter(
        ([key]) =>
          !key.startsWith('node/')
          && key !== 'total'
          && !key.includes('cache')
          && !viewerKeys.includes(key)
      )
      .sort((a, b) => b[1] - a[1]);
  });

  const nodePerformanceData = $derived.by(() => {
    const source = showAverage ? averages : lasts;
    return Object.entries(source)
      .filter(([key]) => key.startsWith('node/'))
      .sort((a, b) => b[1] - a[1]);
  });

  const viewerPerformanceData = $derived.by(() => {
    const source = showAverage ? averages : lasts;
    return Object.entries(source)
      .filter(
        ([key]) =>
          key !== 'total-vertices'
          && key !== 'total-faces'
          && viewerKeys.includes(key)
      )
      .sort((a, b) => b[1] - a[1]);
  });

  const splitValues = $derived.by(() => {
    if (showAverage) {
      return [
        getAverage('worker-transfer'),
        getAverage('runtime'),
        getAverage('update-geometries')
      ];
    }
    return [
      getLast('worker-transfer'),
      getLast('runtime'),
      getLast('update-geometries')
    ];
  });

  const totalPoints = $derived.by(() => {
    if (showAverage) {
      return data.map((run) => {
        return (
          (run['runtime']?.reduce((acc, v) => acc + v, 0) || 0)
          + (run['update-geometries']?.reduce((acc, v) => acc + v, 0) || 0)
          + (run['worker-transfer']?.reduce((acc, v) => acc + v, 0) || 0)
        );
      });
    }

    return data.map((run) => {
      return (
        (run['runtime']?.[0] || 0)
        + (run['update-geometries']?.[0] || 0)
        + (run['worker-transfer']?.[0] || 0)
      );
    });
  });

  function constructPoints(key: string) {
    if (key === 'total') {
      return totalPoints;
    }
    return data.map((run) => {
      if (key in run) {
        if (showAverage) {
          return run[key].reduce((acc, v) => acc + v, 0) / run[key].length;
        } else {
          return run[key][0];
        }
      }
      return 0;
    });
  }

  const computedTotalDisplay = $derived.by(() =>
    round(showAverage ? totalPerformance.average : totalPerformance.onlyLast)
  );

  const computedFps = $derived.by(() =>
    Math.floor(
      1000
        / (showAverage
          ? totalPerformance.average || 1
          : totalPerformance.onlyLast || 1)
    )
  );
</script>

{#if data.length !== 0}
  {#if activeType === 'cache-hit'}
    <Monitor
      title="Cache Hits"
      points={constructPoints(activeType)}
      min={0}
      max={1}
      type="%"
    />
  {:else}
    <Monitor
      title={getTitle(activeType)}
      points={constructPoints(activeType)}
    />
  {/if}

  <div class="p-4 performance-tabler">
    <div class="flex items-center gap-2">
      <Checkbox id="show-total" bind:value={showAverage} />
      <label for="show-total">Show Average</label>
    </div>

    <BarSplit
      labels={['worker-transfer', 'runtime', 'update-geometries']}
      values={splitValues}
    />

    <h3>General</h3>

    <table>
      <tbody>
        <tr>
          <td>
            {computedTotalDisplay}<span>ms</span>
          </td>
          <td
            class:active={activeType === 'total'}
            onclick={() => (activeType = 'total')}
          >
            total<span>({computedFps}fps)</span>
          </td>
        </tr>

        {#each performanceData as [key, value] (key)}
          <tr>
            <td>{round(value)}<span>ms</span></td>
            <td
              class:active={activeType === key}
              onclick={() => (activeType = key)}
            >
              {key}
            </td>
          </tr>
        {/each}

        <tr>
          <td>{data.length}</td>
          <td>Samples</td>
        </tr>
      </tbody>

      <tbody>
        <tr><td><h3>Nodes</h3></td></tr>
      </tbody>

      <tbody>
        <tr>
          <td>{showAverage ? cacheRatio.average : cacheRatio.onlyLast}<span>%</span></td>
          <td
            class:active={activeType === 'cache-hit'}
            onclick={() => (activeType = 'cache-hit')}
          >
            cache hits
          </td>
        </tr>

        {#each nodePerformanceData as [key, value] (key)}
          <tr>
            <td>{round(value)}<span>ms</span></td>
            <td
              class:active={activeType === key}
              onclick={() => (activeType = key)}
            >
              {key.split('/').slice(-1).join('/')}
            </td>
          </tr>
        {/each}
      </tbody>

      <tbody>
        <tr><td><h3>Viewer</h3></td></tr>
      </tbody>

      <tbody>
        <tr>
          <td>{humanizeNumber(getLast('total-vertices'))}</td>
          <td>Vertices</td>
        </tr>
        <tr>
          <td>{humanizeNumber(getLast('total-faces'))}</td>
          <td>Faces</td>
        </tr>

        {#each viewerPerformanceData as [key, value] (key)}
          <tr>
            <td>{round(value)}<span>ms</span></td>
            <td
              class:active={activeType === key}
              onclick={() => (activeType = key)}
            >
              {key.split('/').slice(-1).join('/')}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  <p>No runs available</p>
{/if}

<style>
  h3 {
    margin: 0;
    margin-top: 1em;
    margin-bottom: 0.2em;
    margin-left: 3px;
  }
  span {
    opacity: 0.3;
    margin-left: 4px;
  }
  table {
    margin-bottom: 70px;
  }
  td {
    padding-right: 10px;
    padding-block: 5px;
  }
  td.active {
    font-weight: bold;
  }
  tr > td:nth-child(1) {
    text-align: right;
  }
  tr > td:nth-child(2) {
    opacity: 0.5;
  }
</style>
