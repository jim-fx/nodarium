<script lang="ts">
  import { curvePath } from '../helpers/curve';

  type Props = {
    // [x0, y0, x1, y1, ...] from 0 to 1, sorted by x
    value: number[];
  };

  let { value: points = $bindable([0, 0, 1, 1]) }: Props = $props();

  // the closest two points can get, so they never swap their order
  const GAP = 0.01;

  let svgElement = $state<SVGSVGElement>(null!);
  let draggingIndex = $state<number>();
  let mouseDown: [number, number] | undefined;
  let downPosition: [number, number] | undefined;
  let svgRect: DOMRect;

  const groupedPoints = $derived(
    Array.from({ length: points.length / 2 }, (_, i) => [points[i * 2], points[i * 2 + 1]])
  );
  const linePath = $derived(curvePath(points));
  const areaPath = $derived(linePath && `${linePath} L 100 100 L 0 100 Z`);

  const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);
  const round = (v: number) => Math.round(v * 1000) / 1000;

  $effect(() => {
    if (points.length < 4) {
      points = [0, 0, 1, 1];
    }
  });

  // the first and last point stay at the edges, the others between their neighbours
  function setPoint(index: number, x: number, y: number) {
    const last = points.length / 2 - 1;
    if (index === 0) {
      x = 0;
    } else if (index === last) {
      x = 1;
    } else {
      x = clamp(x, points[index * 2 - 2] + GAP, points[index * 2 + 2] - GAP);
    }
    points[index * 2] = round(x);
    points[index * 2 + 1] = round(clamp(y, 0, 1));
  }

  function handleMouseMove(ev: MouseEvent) {
    if (mouseDown === undefined || draggingIndex === undefined || !downPosition) {
      return;
    }

    let dx = (ev.clientX - mouseDown[0]) / svgRect.width;
    let dy = (mouseDown[1] - ev.clientY) / svgRect.height;

    if (ev.shiftKey) {
      dx /= 10;
      dy /= 10;
    }

    setPoint(draggingIndex, downPosition[0] + dx, downPosition[1] + dy);
  }

  function handleMouseDown(ev: MouseEvent) {
    if (ev.button !== 0) return;
    ev.preventDefault();

    svgRect = svgElement.getBoundingClientRect();
    mouseDown = [ev.clientX, ev.clientY];

    const indexText = (ev.target as SVGCircleElement).dataset.index;
    if (indexText !== undefined) {
      draggingIndex = parseInt(indexText);
    } else {
      const x = (ev.clientX - svgRect.left) / svgRect.width;
      const y = 1 - (ev.clientY - svgRect.top) / svgRect.height;

      let index = 1;
      while (index < points.length / 2 - 1 && points[index * 2] < x) index++;
      points.splice(index * 2, 0, x, y);
      setPoint(index, x, y);
      draggingIndex = index;
    }

    downPosition = [points[draggingIndex * 2], points[draggingIndex * 2 + 1]];
  }

  function handleMouseUp() {
    mouseDown = undefined;
    draggingIndex = undefined;
  }

  function handleContextMenu(ev: MouseEvent) {
    const indexText = (ev.target as SVGCircleElement).dataset?.index;
    if (indexText === undefined) return;

    ev.preventDefault();
    ev.stopImmediatePropagation();

    const index = parseInt(indexText);
    if (index > 0 && index < points.length / 2 - 1) {
      draggingIndex = undefined;
      points.splice(index * 2, 2);
    }
  }
</script>

<svelte:window onmousemove={handleMouseMove} onmouseup={handleMouseUp} />

<div class="w-full aspect-square bg-layer-2 p-[7px] rounded-[5px] outline-1 outline-outline">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <svg
    class="group size-full overflow-visible"
    viewBox="0 0 100 100"
    bind:this={svgElement}
    aria-label="Curve editor"
    onmousedown={handleMouseDown}
    oncontextmenu={handleContextMenu}
  >
    <path d={areaPath} class="fill-layer-3 opacity-30" />
    <path d={linePath} class="fill-none stroke-layer-3" />
    {#each groupedPoints as p, i (i)}
      <circle
        class="cursor-pointer stroke-layer-3 opacity-0 transition-opacity group-hover:opacity-100 hover:fill-layer-3"
        class:fill-layer-3={draggingIndex === i}
        class:fill-layer-2={draggingIndex !== i}
        data-index={i}
        cx={p[0] * 100}
        cy={100 - p[1] * 100}
        r={3}
      />
    {/each}
  </svg>
</div>
