<script lang="ts">
  import { T } from '@threlte/core';
  import { type Group, Vector3 } from 'three';
  import { viewScale } from '../scale/view-scale.svelte';
  import { GIZMO_STYLE } from './gizmo-style';
  import GizmoControls from './GizmoControls.svelte';
  import GizmoLine from './GizmoLine.svelte';
  import {
    type Gizmo,
    type GizmoHandlers,
    MAX_BRANCH_ANGLE,
    MIN_LENGTH,
    roundGizmoValue,
    type Vec3
  } from './gizmos';
  import GizmoSphere from './GizmoSphere.svelte';

  type Props = GizmoHandlers & { gizmo: Extract<Gizmo, { kind: 'branch' }> };

  const { gizmo, onchange, onstart, onend }: Props = $props();

  const origin = $derived(new Vector3(...gizmo.origin));
  const direction = $derived(new Vector3(...gizmo.direction));
  const orthogonal = $derived(new Vector3(...gizmo.orthogonal));

  function tipAt(angle: number, length: number) {
    const radians = (angle * Math.PI) / 180;
    return origin
      .clone()
      .addScaledVector(orthogonal, Math.cos(radians) * length)
      .addScaledVector(direction, Math.sin(radians) * length);
  }

  const tip = $derived(tipAt(gizmo.angle, gizmo.length).toArray() as Vec3);

  let group = $state<Group>();

  // The branch leans in the plane of the path and the orthogonal, wherever the handle is dragged
  // it is projected into that plane.
  function handleChange() {
    if (!group) return;
    const offset = group.position.clone().sub(origin);
    const x = offset.dot(orthogonal);
    const y = offset.dot(direction);

    const length = Math.max(Math.hypot(x, y), MIN_LENGTH);
    const degrees = (Math.atan2(y, x) * 180) / Math.PI;
    const angle = Math.min(Math.max(degrees, -MAX_BRANCH_ANGLE), MAX_BRANCH_ANGLE);

    group.position.copy(tipAt(angle, length));
    onchange?.(gizmo.nodeId, {
      angle: roundGizmoValue(angle),
      length: roundGizmoValue(length)
    });
  }
</script>

<GizmoLine from={gizmo.origin} to={tip} />

<T.Group position={gizmo.origin} scale={GIZMO_STYLE.markerSize * viewScale.value}>
  <GizmoSphere opacity={GIZMO_STYLE.markerOpacity} />
</T.Group>

<GizmoControls
  bind:group
  mode="translate"
  position={tip}
  scale={GIZMO_STYLE.markerSize * viewScale.value}
  onmouseDown={() => onstart?.()}
  onmouseUp={() => onend?.()}
  onobjectChange={handleChange}
>
  <GizmoSphere opacity={GIZMO_STYLE.markerOpacity} />
</GizmoControls>
