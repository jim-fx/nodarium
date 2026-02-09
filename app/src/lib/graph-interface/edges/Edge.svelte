<script module lang="ts">
  import { colors } from '../graph/colors.svelte';

  const circleMaterial = new MeshBasicMaterial({
    color: colors.outline.clone(),
    toneMapped: false
  });

  let lineColor = $state(colors.outline.clone().convertSRGBToLinear());

  $effect.root(() => {
    $effect(() => {
      if (appSettings.value.theme === undefined) {
        return;
      }
      circleMaterial.color = colors.outline.clone().convertSRGBToLinear();
      lineColor = colors.outline.clone().convertSRGBToLinear();
    });
  });

  const curve = new CubicBezierCurve(
    new Vector2(0, 0),
    new Vector2(0, 0),
    new Vector2(0, 0),
    new Vector2(0, 0)
  );
</script>

<script lang="ts">
  import { appSettings } from '$lib/settings/app-settings.svelte';
  import { T } from '@threlte/core';
  import { MeshLineGeometry, MeshLineMaterial } from '@threlte/extras';
  import { onDestroy } from 'svelte';
  import { MeshBasicMaterial, Vector3 } from 'three';
  import { CubicBezierCurve } from 'three/src/extras/curves/CubicBezierCurve.js';
  import { Vector2 } from 'three/src/math/Vector2.js';
  import { getGraphState } from '../graph-state.svelte';

  const graphState = getGraphState();

  type Props = {
    x1: number;
    y1: number;
    x2: number;
    y2: number;
    z: number;
    id?: string;
  };

  const { x1, y1, x2, y2, z, id }: Props = $props();

  const thickness = $derived(Math.max(0.001, 0.00082 * Math.exp(0.055 * z)));

  let points = $state<Vector3[]>([]);

  let lastId: string | null = null;
  const curveId = $derived(`${x1}-${y1}-${x2}-${y2}`);

  function update() {
    const new_x = x2 - x1;
    const new_y = y2 - y1;
    if (lastId === curveId) {
      return;
    }
    lastId = curveId;

    const length = Math.floor(
      Math.sqrt(Math.pow(new_x, 2) + Math.pow(new_y, 2)) / 4
    );

    const samples = Math.max(length * 16, 10);

    curve.v0.set(0, 0);
    curve.v1.set(new_x / 2, 0);
    curve.v2.set(new_x / 2, new_y);
    curve.v3.set(new_x, new_y);

    points = curve
      .getPoints(samples)
      .map((p) => new Vector3(p.x, 0, p.y))
      .flat();

    if (id) {
      graphState.setEdgeGeometry(
        id,
        x1,
        y1,
        $state.snapshot(points) as unknown as Vector3[]
      );
    }
  }

  $effect(() => {
    if (x1 || x2 || y1 || y2) {
      update();
    }
  });

  onDestroy(() => {
    if (id) graphState.removeEdgeGeometry(id);
  });
</script>

<T.Mesh
  position.x={x1}
  position.z={y1}
  position.y={0.8}
  rotation.x={-Math.PI / 2}
  material={circleMaterial}
>
  <T.CircleGeometry args={[0.5, 16]} />
</T.Mesh>

<T.Mesh
  position.x={x2}
  position.z={y2}
  position.y={0.8}
  rotation.x={-Math.PI / 2}
  material={circleMaterial}
>
  <T.CircleGeometry args={[0.5, 16]} />
</T.Mesh>

{#if graphState.hoveredEdgeId === id}
  <T.Mesh position.x={x1} position.z={y1} position.y={0.1}>
    <MeshLineGeometry {points} />
    <MeshLineMaterial
      width={thickness * 5}
      color={lineColor}
      opacity={0.5}
      transparent
    />
  </T.Mesh>
{/if}

<T.Mesh position.x={x1} position.z={y1} position.y={0.1}>
  <MeshLineGeometry {points} />
  <MeshLineMaterial width={thickness} color={lineColor} />
</T.Mesh>
