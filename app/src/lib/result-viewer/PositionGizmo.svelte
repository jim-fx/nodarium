<script lang="ts">
  import type { Group } from 'three';
  import { GIZMO_STYLE } from './gizmo-style';
  import GizmoControls from './GizmoControls.svelte';
  import {
    type Gizmo,
    type GizmoHandlers,
    type GizmoMode,
    radiusFromScale,
    roundGizmoValue,
    type Vec3
  } from './gizmos';
  import GizmoSphere from './GizmoSphere.svelte';

  type Props = GizmoHandlers & {
    gizmo: Extract<Gizmo, { kind: 'force' | 'position' }>;
    mode: GizmoMode;
  };

  const { gizmo, mode, onchange, onstart, onend }: Props = $props();

  // a plain position has no radius to scale
  const activeMode = $derived<GizmoMode>(gizmo.kind === 'force' ? mode : 'translate');
  const radius = $derived(gizmo.kind === 'force' ? gizmo.radius : GIZMO_STYLE.positionSize);

  let group = $state<Group>();
  let startRadius = 1;

  function handleStart() {
    startRadius = radius;
    onstart?.();
  }

  function handleChange() {
    if (!group) return;

    if (activeMode === 'scale') {
      const { x, y, z } = group.scale;
      const next = radiusFromScale([x, y, z], startRadius);
      // the sphere keeps its shape, no matter which axis was dragged
      group.scale.setScalar(next);
      onchange?.(gizmo.nodeId, { radius: roundGizmoValue(next) });
      return;
    }

    const position = group.position.toArray().map(roundGizmoValue) as Vec3;
    // a vec3 node keeps its position in three inputs
    onchange?.(
      gizmo.nodeId,
      gizmo.kind === 'force'
        ? { position }
        : { '0': position[0], '1': position[1], '2': position[2] }
    );
  }
</script>

<GizmoControls
  bind:group
  mode={activeMode}
  position={gizmo.position}
  scale={radius}
  onmouseDown={handleStart}
  onmouseUp={() => onend?.()}
  onobjectChange={handleChange}
>
  <GizmoSphere
    opacity={gizmo.kind === 'force' ? GIZMO_STYLE.forceOpacity : GIZMO_STYLE.markerOpacity}
  />
</GizmoControls>
