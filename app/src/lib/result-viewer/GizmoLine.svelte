<script lang="ts">
  import { T } from '@threlte/core';
  import { Quaternion, Vector3 } from 'three';
  import { GIZMO_STYLE } from './gizmo-style';
  import { gizmoTheme } from './gizmo-theme.svelte.ts';
  import type { Vec3 } from './gizmos';

  type Props = { from: Vec3; to: Vec3 };

  const { from, to }: Props = $props();

  const UP = new Vector3(0, 1, 0);

  const direction = $derived(new Vector3(...to).sub(new Vector3(...from)));
  const length = $derived(direction.length());
  const middle = $derived<Vec3>([
    (from[0] + to[0]) / 2,
    (from[1] + to[1]) / 2,
    (from[2] + to[2]) / 2
  ]);
  const rotation = $derived(
    new Quaternion().setFromUnitVectors(UP, direction.clone().normalize()).toArray() as [
      number,
      number,
      number,
      number
    ]
  );
</script>

{#if length > 0.0001}
  <T.Mesh
    position={middle}
    quaternion={rotation}
    scale={[1, length, 1]}
    renderOrder={GIZMO_STYLE.renderOrder}
  >
    <T.CylinderGeometry args={[GIZMO_STYLE.lineThickness, GIZMO_STYLE.lineThickness, 1, 6]} />
    <T.MeshBasicMaterial color={gizmoTheme.accent} depthWrite={false} depthTest={false} />
  </T.Mesh>
{/if}
