<script lang="ts">
  import { colors } from '#lib/graph-interface/graph/colors.svelte.ts';
  import { T } from '@threlte/core';
  import { TransformControls } from '@threlte/extras';
  import type { ColorRepresentation, Group } from 'three';
  import {
    type ForceGizmo,
    type GizmoChange,
    type GizmoMode,
    radiusFromScale,
    roundGizmoValue,
    type Vec3
  } from './gizmos';

  type Props = {
    gizmos: ForceGizmo[];
    mode: GizmoMode;
    onchange?: (nodeId: number, change: GizmoChange) => void;
    onstart?: () => void;
    onend?: () => void;
  };

  const { gizmos, mode, onchange, onstart, onend }: Props = $props();

  // drawn on top of the result, like the debug view
  const RENDER_ORDER = 999;

  // only the active node has a gizmo
  const active = $derived(gizmos[0]);
  // a plain position has no radius to scale
  const activeMode = $derived<GizmoMode>(active?.radius === undefined ? 'translate' : mode);
  // the size of the marker of a position without a radius
  const POSITION_MARKER = 0.12;

  let group = $state<Group>();
  let startRadius = 1;

  function handleStart() {
    startRadius = active?.radius ?? 1;
    onstart?.();
  }

  function handleChange() {
    if (!group || !active) return;

    if (activeMode === 'scale') {
      const { x, y, z } = group.scale;
      const radius = radiusFromScale([x, y, z], startRadius);
      // the sphere keeps its shape, no matter which axis was dragged
      group.scale.setScalar(radius);
      onchange?.(active.nodeId, { radius: roundGizmoValue(radius) });
    } else {
      const position = group.position.toArray().map(roundGizmoValue) as Vec3;
      onchange?.(active.nodeId, { position });
    }
  }
</script>

{#snippet sphere(opacity: number, color: ColorRepresentation)}
  <T.Mesh renderOrder={RENDER_ORDER}>
    <T.SphereGeometry args={[1, 32, 16]} />
    <T.MeshBasicMaterial {color} transparent {opacity} depthWrite={false} depthTest={false} />
  </T.Mesh>
  <T.Mesh renderOrder={RENDER_ORDER}>
    <T.SphereGeometry args={[1, 16, 8]} />
    <T.MeshBasicMaterial
      {color}
      wireframe
      transparent
      opacity={opacity * 2}
      depthWrite={false}
      depthTest={false}
    />
  </T.Mesh>
{/snippet}

{#if active}
  {#key active.nodeId}
    <TransformControls
      bind:group
      mode={activeMode}
      position={active.position}
      scale={active.radius ?? POSITION_MARKER}
      onmouseDown={handleStart}
      onmouseUp={() => onend?.()}
      onobjectChange={handleChange}
    >
      {@render sphere(active.radius === undefined ? 0.5 : 0.08, colors['selected'])}
    </TransformControls>
  {/key}
{/if}
