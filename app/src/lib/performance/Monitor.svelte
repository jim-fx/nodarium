<script lang="ts">
  type Props = {
    points: number[];
    type?: string;
    title?: string;
    max?: number;
    min?: number;
  };

  let {
    points,
    type = 'ms',
    title = 'Performance',
    max,
    min
  }: Props = $props();

  let internalMax = $derived(max ?? Math.max(...points));
  let internalMin = $derived(min ?? Math.min(...points))!;

  const maxText = $derived.by(() => {
    if (type === '%') {
      return 100;
    }

    if (internalMax !== undefined) {
      if (internalMax < 1) {
        return Math.floor(internalMax * 100) / 100;
      }
      if (internalMax < 10) {
        return Math.floor(internalMax * 10) / 10;
      }
      return Math.floor(internalMax);
    }

    return 1;
  });

  const path = $derived(
    points
      .map((point, i) => {
        const x = (i / (points.length - 1)) * 100;
        const y = 100 - ((point - internalMin) / (internalMax - internalMin)) * 100;
        return `${x},${y}`;
      })
      .join(' ')
  );
</script>

<div class="wrapper">
  <p>{title}</p>
  <span class="min">{Math.floor(internalMin || 0)}{type}</span>
  <span class="max">{maxText}{type}</span>
  <svg preserveAspectRatio="none" viewBox="0 0 100 100">
    <polyline vector-effect="non-scaling-stroke" points={path} />
  </svg>
</div>

<style>
  span {
    position: absolute;
    right: 10px;
    font-size: 0.8em;
    opacity: 0.5;
  }

  .max {
    top: 4px;
  }

  .min {
    bottom: 5px;
  }

  .wrapper {
    position: relative;
    border-bottom: solid thin var(--outline);
    display: flex;
  }
  p {
    margin: 0px;
    top: 3px;
    left: 5px;
    font-size: 0.9em;
    opacity: 0.5;
    position: absolute;
  }
  svg {
    height: 124px;
    margin: 24px 0px;
    border-top: solid thin var(--outline);
    border-bottom: solid thin var(--outline);
    width: 100%;
  }
  polyline {
    fill: none;
    stroke: var(--layer-3);
    opacity: 0.5;
    stroke-width: 1;
  }
</style>
