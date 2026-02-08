<script lang="ts">
  let mouseDown = $state<number[]>();
  let draggingIndex = $state<number>();
  let downCirclePosition = $state<number[]>();
  let svgElement = $state<SVGElement>(null!);
  let svgRect = $state<DOMRect>(null!);

  type Props = {
    value: number[];
    mirror?: boolean;
  };

  function defaultPoints(): number[] {
    return [47.8, 100, 47.8, 82.8, 30.9, 69.1, 23.2, 40.7, 27.1, 14.5, 42.5, 0];
  }

  let { value: points = $bindable(), mirror = true }: Props = $props();

  const getPt = (pts: number[], i: number) => [pts[i * 2], pts[i * 2 + 1]] as [number, number];
  const setPt = (pts: number[], i: number, x: number, y: number) => {
    pts[i * 2] = x;
    pts[i * 2 + 1] = y;
  };

  $effect(() => {
    if (!points.length) {
      points = defaultPoints();
    }
  });

  function mirrorPoints(pts: number[]) {
    const res: number[] = [];
    for (let i = 0; i < pts.length; i += 2) {
      res.push(100 - pts[i], pts[i + 1]);
    }
    return res;
  }

  function calculatePath(pts: number[], mirror = false): string {
    if (pts.length === 0) return '';

    const arr = [...pts];
    if (mirror) {
      const sorted = [];
      for (let i = 0; i < arr.length; i += 2) {
        sorted.push({ x: arr[i], y: arr[i + 1], idx: i });
      }
      sorted.sort((a, b) => {
        if (a.y !== b.y) return b.y - a.y;
        return a.x - b.x;
      });
      for (let i = 0; i < sorted.length; i++) {
        arr[i * 2] = sorted[i].x;
        arr[i * 2 + 1] = sorted[i].y;
      }
    }

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

  const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);
  const round = (v: number) => Math.floor(v * 10) / 10;

  function handleMouseMove(ev: MouseEvent) {
    if (mouseDown === undefined || draggingIndex === undefined || !downCirclePosition) return;

    let vx = (mouseDown[0] - ev.clientX) * (100 / svgRect.width);
    let vy = (mouseDown[1] - ev.clientY) * (100 / svgRect.height);

    let x = downCirclePosition[0] - vx;
    let y = downCirclePosition[1] - vy;

    x = clamp(x, 0, mirror ? 50 : 100);
    y = clamp(y, 0, 100);

    setPt(points, draggingIndex, round(x), round(y));
  }

  function clientToSvg(ev: MouseEvent): [number, number] {
    svgRect = svgElement.getBoundingClientRect();

    const x = ((ev.clientX - svgRect.left) / svgRect.width) * 100;
    const y = ((ev.clientY - svgRect.top) / svgRect.height) * 100;

    return [round(clamp(x, 0, mirror ? 50 : 100)), round(clamp(y, 0, 100))];
  }

  function dist(a: [number, number], b: [number, number]) {
    return Math.hypot(a[0] - b[0], a[1] - b[1]);
  }

  function insertBetween(newPt: [number, number]) {
    const count = points.length / 2;
    if (count < 2) {
      points = [...points, newPt[0], newPt[1]];
      return;
    }

    let minDist = Infinity;
    let insertIdx = 0;

    for (let i = 0; i < count - 1; i++) {
      const a = getPt(points, i);
      const b = getPt(points, i + 1);
      const d = dist(newPt, a) + dist(newPt, b) - dist(a, b);
      if (d < minDist) {
        minDist = d;
        insertIdx = i + 1;
      }
    }

    points.splice(insertIdx * 2, 0, newPt[0], newPt[1]);
  }

  function handleMouseDown(ev: MouseEvent) {
    ev.preventDefault();

    svgRect = svgElement.getBoundingClientRect();
    mouseDown = [ev.clientX, ev.clientY];

    const target = ev.target as SVGCircleElement;
    const index = target.dataset?.index;

    if (index !== undefined) {
      draggingIndex = parseInt(index);
      downCirclePosition = [...getPt(points, draggingIndex)];
    } else {
      const pt = clientToSvg(ev);
      if (mirror && pt[0] > 50) return;
      insertBetween(pt);
      const count = points.length / 2;
      draggingIndex = count - 1;
      downCirclePosition = [...pt];
    }
  }

  function handleMouseUp(ev: MouseEvent) {
    mouseDown = undefined;
    if ((ev.target as HTMLElement).dataset?.index === undefined) {
      draggingIndex = undefined;
    }
    console.log('MouseUp', ev.target);
  }

  function handleKeyDown(ev: KeyboardEvent) {
    if (draggingIndex === undefined) return;
    if (ev.key === 'Delete' || ev.key === 'Backspace') {
      ev.preventDefault();
      ev.stopImmediatePropagation();
      points.splice(draggingIndex * 2, 2);
      points = [...points];
      draggingIndex = undefined;
    }
  }
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} onkeydown={handleKeyDown} />

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
    <path d={calculatePath(points, mirror)} style:fill="var(--color-layer-3)" style:opacity={0.5} />
    <path d={calculatePath(points, mirror)} fill="none" stroke="var(--color-layer-2)" />
    {#if mirror}
      <!-- eslint-disable-next-line @typescript-eslint/no-unused-vars -->
      {#each Array(points.length / 2) as _, i (i)}
        {@const x = mirrorPoints(points)[i * 2]}
        {@const y = mirrorPoints(points)[i * 2 + 1]}
        <circle
          class="disabled"
          cx={x}
          cy={y}
          r={2}
        >
        </circle>
      {/each}
    {/if}

    <!-- eslint-disable-next-line @typescript-eslint/no-unused-vars -->
    {#each Array(points.length / 2) as _, i (i)}
      {@const x = points[i * 2]}
      {@const y = points[i * 2 + 1]}
      <circle
        class:active={draggingIndex === i}
        class="interactive"
        data-index={i}
        cx={x}
        cy={y}
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
  }

  circle.active {
    stroke: var(--color-layer-3);
    stroke-width: 0.5px;
  }

  circle.disabled {
    pointer-events: none;
    z-index: -1;
  }
</style>
