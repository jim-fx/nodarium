<script lang="ts">
  type Vec2 = [
    number,
    number
  ];

  let mouseDown = $state<Vec2>();
  let activeCircle = $state<number>();
  let downCirclePosition = $state<Vec2>();
  let svgElement = $state<SVGElement>(null!);
  let svgRect = $state<DOMRect>(null!);

  type Props = {
    points: Vec2[];
    mirror?: boolean;
  };

  function defaultPoints(): Vec2[] {
    return [
      [0, 0],
      [10, 10]
    ];
  }

  let { points = $bindable(), mirror = true }: Props = $props();

  $effect(() => {
    if (!points.length) {
      points = defaultPoints();
    }
  });

  function mirrorPoints(pts: Vec2[]) {
    const _pts: Vec2[] = [];

    for (const pt of pts) {
      _pts.push([
        100 - pt[0],
        pt[1]
      ]);
    }

    return _pts;
  }

  $effect(() => {
    const pts = $state.snapshot(points)
      .map((p, i) => [...p, i])
      .sort((a, b) => {
        if (a[1] !== b[1]) return b[1] - a[1];
        return a[0] - b[0];
      })
      .map((p, i) => [...p, i]);

    let diff = pts.find((p, i) => p[0] !== points[i][0] || p[1] !== points[i][1]);
    if (diff) {
      points = pts.map(p => [p[0], p[1]]);
      const newActiveCircle = pts.find(p => p[2] === activeCircle);
      if (newActiveCircle) {
        activeCircle = newActiveCircle[3];
      }
    }
  });

  function calculatePath(points: Vec2[], mirror = false): string {
    if (points.length === 0) return '';

    const pts = $state.snapshot(points);
    if (mirror) {
      pts.sort((a, b) => (a[1] > b[1] ? -1 : 1));
    }

    let d = `M ${pts[0][0]} ${pts[0][1]}`;

    for (let i = 1; i < pts.length; i++) {
      d += ` L ${pts[i][0]} ${pts[i][1]}`;
    }

    if (mirror) {
      for (let i = pts.length - 1; i >= 0; i--) {
        const p = pts[i];
        const x = mirror ? 100 - p[0] : p[0];
        d += ` L ${x} ${p[1]}`;
      }
      d += ' Z';
    }

    return d;
  }

  const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);
  const round = (v: number) => Math.floor(v * 10) / 10;

  function handleMouseMove(ev: MouseEvent) {
    if (mouseDown === undefined || activeCircle === undefined || !downCirclePosition) return;

    let vx = (mouseDown[0] - ev.clientX) * (100 / svgRect.width);
    let vy = (mouseDown[1] - ev.clientY) * (100 / svgRect.height);

    let x = downCirclePosition[0] - vx;
    let y = downCirclePosition[1] - vy;

    x = clamp(x, 0, mirror ? 50 : 100);
    y = clamp(y, 0, 100);

    points[activeCircle][0] = round(x);
    points[activeCircle][1] = round(y);
  }

  function clientToSvg(ev: MouseEvent): Vec2 {
    svgRect = svgElement.getBoundingClientRect();

    const x = ((ev.clientX - svgRect.left) / svgRect.width) * 100;
    const y = ((ev.clientY - svgRect.top) / svgRect.height) * 100;

    return [
      round(clamp(x, 0, mirror ? 50 : 100)),
      round(clamp(y, 0, 100))
    ];
  }

  function dist(a: Vec2, b: Vec2) {
    return Math.hypot(a[0] - b[0], a[1] - b[1]);
  }

  function insertBetween(newPt: Vec2) {
    if (points.length < 2) {
      points = [...points, newPt];
      return;
    }

    let minDist = Infinity;
    let insertIdx = 0;

    for (let i = 0; i < points.length - 1; i++) {
      const d = dist(newPt, points[i]) + dist(newPt, points[i + 1])
        - dist(points[i], points[i + 1]);
      if (d < minDist) {
        minDist = d;
        insertIdx = i + 1;
      }
    }

    points.splice(insertIdx, 0, newPt);
  }

  function handleMouseDown(ev: MouseEvent) {
    ev.preventDefault();

    svgRect = svgElement.getBoundingClientRect();
    mouseDown = [ev.clientX, ev.clientY];

    const target = ev.target as SVGCircleElement;
    const index = target.dataset?.index;

    if (index !== undefined) {
      activeCircle = parseInt(index);
      downCirclePosition = [...points[activeCircle]];
    } else {
      const pt = clientToSvg(ev);
      if (mirror && pt[0] > 50) return;
      insertBetween(pt);
      activeCircle = points.findIndex(p => p[0] === pt[0] && p[1] === pt[1]);
      downCirclePosition = [...pt];
    }
  }

  function handleMouseUp() {
    mouseDown = undefined;
  }

  function handleKeyDown(ev: KeyboardEvent) {
    if (activeCircle === undefined) return;
    if (ev.key === 'Delete' || ev.key === 'Backspace') {
      const pts = [...points];
      pts.splice(activeCircle, 1);
      points = pts;
      activeCircle = undefined;
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
      {#each mirrorPoints(points) as point, i (i + '-' + point[0] + '-' + point[1])}
        <circle
          class="disabled"
          cx={point[0]}
          cy={point[1]}
          r={2}
        >
        </circle>
      {/each}
    {/if}

    {#each points as point, i (i + '-' + point[0] + '-' + point[1])}
      <circle
        class:active={activeCircle === i}
        class="interactive"
        data-index={i}
        cx={point[0]}
        cy={point[1]}
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
