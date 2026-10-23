<script lang="ts">
  import type { Group } from 'three';
  import { GIZMO_STYLE } from './gizmo-style';
  import GizmoControls from './GizmoControls.svelte';
  import GizmoLine from './GizmoLine.svelte';
  import { type Gizmo, type GizmoHandlers, MIN_LENGTH, roundGizmoValue, type Vec3 } from './gizmos';
  import GizmoSphere from './GizmoSphere.svelte';
  import { viewScale } from './view-scale.svelte';

  type Props = GizmoHandlers & { gizmo: Extract<Gizmo, { kind: 'stem' }> };

  const { gizmo, onchange, onstart, onend }: Props = $props();

  // the stem grows straight up from its origin
  const tip = $derived<Vec3>([
    gizmo.origin[0],
    gizmo.origin[1] + (gizmo.length ?? 0),
    gizmo.origin[2]
  ]);

  let baseGroup = $state<Group>();
  let tipGroup = $state<Group>();

  function moveBase() {
    if (!baseGroup) return;
    const origin = baseGroup.position.toArray().map(roundGizmoValue) as Vec3;
    onchange?.(gizmo.nodeId, { origin });
  }

  function moveTip() {
    if (!tipGroup) return;
    const length = Math.max(tipGroup.position.y - gizmo.origin[1], MIN_LENGTH);
    onchange?.(gizmo.nodeId, { length: roundGizmoValue(length) });
  }
</script>

<GizmoControls
  bind:group={baseGroup}
  mode="translate"
  position={gizmo.origin}
  scale={GIZMO_STYLE.markerSize * viewScale.value}
  onmouseDown={() => onstart?.()}
  onmouseUp={() => onend?.()}
  onobjectChange={moveBase}
>
  <GizmoSphere opacity={GIZMO_STYLE.markerOpacity} />
</GizmoControls>

{#if gizmo.length !== undefined}
  <GizmoLine from={gizmo.origin} to={tip} />

  <!-- only up and down, the length is all there is to set -->
  <GizmoControls
    bind:group={tipGroup}
    mode="translate"
    showX={false}
    showZ={false}
    position={tip}
    scale={GIZMO_STYLE.markerSize * viewScale.value}
    onmouseDown={() => onstart?.()}
    onmouseUp={() => onend?.()}
    onobjectChange={moveTip}
  >
    <GizmoSphere opacity={GIZMO_STYLE.markerOpacity} />
  </GizmoControls>
{/if}
