<script lang="ts">
  import { HTML } from '@threlte/extras';

  type Props = {
    p1: { x: number; y: number };
    p2: { x: number; y: number };
    cameraPosition: [number, number, number];
  };

  const {
    p1 = { x: 0, y: 0 },
    p2 = { x: 0, y: 0 },
    cameraPosition = [0, 1, 0]
  }: Props = $props();

  const width = $derived(Math.abs(p1.x - p2.x) * cameraPosition[2]);
  const height = $derived(Math.abs(p1.y - p2.y) * cameraPosition[2]);

  const x = $derived(Math.max(p1.x, p2.x) - width / cameraPosition[2]);
  const y = $derived(Math.max(p1.y, p2.y) - height / cameraPosition[2]);
</script>

<HTML position.x={x} position.z={y} transform={false}>
  <div
    class="box-selection"
    style={`width: ${width}px; height: ${height}px;`}
  >
  </div>
</HTML>

<style>
  .box-selection {
    width: 40px;
    height: 20px;
    border: solid 2px var(--color-outline);
    border-style: dashed;
    border-radius: 2px;
  }
</style>
