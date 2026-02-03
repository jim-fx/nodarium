<script lang="ts">
  const { points }: { points: number[] } = $props();

  const path = $derived.by(() => {
    const max = Math.max(...points);
    const min = Math.min(...points);
    return points
      .map((point, i) => {
        const x = (i / (points.length - 1)) * 100;
        const y = 100 - ((point - min) / (max - min)) * 100;
        return `${x},${y}`;
      })
      .join(' ');
  });
</script>

<svg preserveAspectRatio="none" viewBox="0 0 100 100">
  <polyline vector-effect="non-scaling-stroke" points={path} />
</svg>

<style>
  svg {
    height: 40px;
    width: 100%;
  }
  polyline {
    fill: none;
    stroke: var(--color-layer-3);
    opacity: 1;
    stroke-width: 1;
  }
</style>
