<script lang="ts">
  type Props = {
    value: number[];
    mirror?: boolean;
  };

  let { value: points = $bindable(), mirror = true }: Props = $props();

  let mouseDown = $state<number[]>();
  let draggingIndex = $state<number>();
  let downCirclePosition = $state<number[]>();
  let svgElement = $state<SVGElement>(null!);
  let svgRect = $state<DOMRect>(null!);
  let isMirroredEvent = $state(false);

  const pathD = $derived(calculatePath(points, mirror));
  const groupedPoints = $derived(group(points));

  function group<T>(arr: T[], size = 2): T[][] {
    const result = [];
    for (let i = 0; i < arr.length; i += size) {
      result.push(arr.slice(i, i + size));
    }
    return result;
  }
  const dist = (a: [number, number], b: [number, number]) => Math.hypot(a[0] - b[0], a[1] - b[1]);
  const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);
  const round = (v: number) => Math.floor(v * 10) / 10;
  const getPt = (i: number) => [points[i * 2], points[i * 2 + 1]] as [number, number];

  $effect(() => {
    if (!points.length) {
      points = [47.8, 100, 47.8, 82.8, 30.9, 69.1, 23.2, 40.7, 27.1, 14.5, 42.5, 0];
    }
  });

  $effect(() => {
    if (mirror) {
      const _points: [number, number, number][] = [];
      for (let i = 0; i < points.length / 2; i++) {
        _points.push([...getPt(i), i]);
      }

      const sortedPoints = _points
        .sort((a, b) => {
          if (a[1] !== b[1]) return b[1] - a[1];
          return a[0] - b[0];
        });

      const newIndices = new Map(sortedPoints.map((p, i) => [p[2], i]));

      const sorted = sortedPoints
        .map(p => [p[0], p[1]])
        .flat();

      let sortChanged = false;

      for (let i = 0; i < sorted.length; i++) {
        if (sorted[i] !== points[i]) {
          sortChanged = true;
          break;
        }
      }

      if (sortChanged) {
        points = sorted;
        draggingIndex = newIndices.get(draggingIndex || 0) || 0;
      }
    }
  });

  function insertBetween(newPt: [number, number]): number {
    const count = points.length / 2;

    if (count < 2) {
      points = [...points, ...newPt];
      return count;
    }

    let minDist = Infinity;
    let insertIdx = 0;

    for (let i = 0; i < count - 1; i++) {
      const a = getPt(i);
      const b = getPt(i + 1);
      const d = dist(newPt, a) + dist(newPt, b) - dist(a, b);
      if (d < minDist) {
        minDist = d;
        insertIdx = i + 1;
      }
    }

    points.splice(insertIdx * 2, 0, newPt[0], newPt[1]);
    return insertIdx;
  }

  function calculatePath(pts: number[], mirror = false): string {
    if (pts.length === 0) return '';

    const arr = [...pts];

    let d = `M ${arr[0]} ${arr[1]}`;
    for (let i = 2; i < arr.length; i += 2) {
      d += ` L ${arr[i]} ${arr[i + 1]}`;
    }

    if (mirror) {
      for (let i = arr.length - 2; i >= 0; i -= 2) {
        const x = 100 - arr[i];
        d += ` L ${x} ${arr[i + 1]}`;
      }
      d += ' Z';
    }

    return d;
  }

  function handleMouseMove(ev: MouseEvent) {
    if (mouseDown === undefined || draggingIndex === undefined || !downCirclePosition) return;

    let vx = (mouseDown[0] - ev.clientX) * (100 / svgRect.width);
    let vy = (mouseDown[1] - ev.clientY) * (100 / svgRect.height);

    let x = downCirclePosition[0] + ((isMirroredEvent ? 1 : -1) * vx);
    let y = downCirclePosition[1] - vy;

    x = clamp(x, 0, mirror ? 50 : 100);
    y = clamp(y, 0, 100);

    points[draggingIndex * 2] = round(x);
    points[draggingIndex * 2 + 1] = round(y);
  }

  function handleMouseDown(ev: MouseEvent) {
    ev.preventDefault();
    isMirroredEvent = false;

    svgRect = svgElement.getBoundingClientRect();
    mouseDown = [ev.clientX, ev.clientY];

    const indexText = (ev.target as SVGCircleElement).dataset.index;

    const x = ((ev.clientX - svgRect.left) / svgRect.width) * 100;
    const y = ((ev.clientY - svgRect.top) / svgRect.height) * 100;
    isMirroredEvent = x > 50;

    if (indexText !== undefined) {
      draggingIndex = parseInt(indexText);
      downCirclePosition = getPt(draggingIndex);
    } else {
      draggingIndex = undefined;

      const pt = [
        round(clamp(x, 0, 100)),
        round(clamp(y, 0, 100))
      ] as [
        number,
        number
      ];
      if (isMirroredEvent) {
        pt[0] = 100 - pt[0];
      }

      draggingIndex = insertBetween(pt);
      downCirclePosition = pt;
    }
  }

  function handleMouseUp() {
    mouseDown = undefined;
    draggingIndex = undefined;
  }

  function handleContextMenu(ev: MouseEvent) {
    const indexText = (ev.target as HTMLElement).dataset?.index;
    if (indexText !== undefined) {
      ev.preventDefault();
      ev.stopImmediatePropagation();
      const index = parseInt(indexText);
      points.splice(index * 2, 2);
      draggingIndex = undefined;
    }
  }
</script>

<svelte:window
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
  oncontextmenu={handleContextMenu}
/>

<div class="wrapper" class:mirrored={mirror}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <svg
    width="100"
    height="100"
    viewBox="0 0 100 100"
    bind:this={svgElement}
    aria-label="Interactive 2D Shape Editor"
    onmousedown={handleMouseDown}
  >
    <path d={pathD} style:fill="var(--color-layer-3)" style:opacity={0.5} />
    <path d={pathD} fill="none" stroke="var(--color-layer-2)" />
    {#if mirror}
      {#each groupedPoints as p, i (i)}
        {@const x = 100 - p[0]}
        {@const y = p[1]}
        <circle
          data-index={i}
          cx={x}
          cy={y}
          r={2}
        >
        </circle>
      {/each}
    {/if}

    {#each groupedPoints as p, i (i)}
      <circle
        data-index={i}
        cx={p[0]}
        cy={p[1]}
        r={2}
      >
      </circle>
    {/each}
  </svg>
</div>

<style>
  .wrapper {
    width: 100%;
    aspect-ratio: 1;
  }

  svg {
    height: 100%;
    width: 100%;
    overflow: visible;
  }

  circle {
    fill: var(--color-layer-2);
    cursor: pointer;
  }
</style>
