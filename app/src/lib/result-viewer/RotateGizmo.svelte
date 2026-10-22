<script lang="ts">
  import { type Group, Quaternion } from 'three';
  import { angleStep } from './gizmo-paths';
  import GizmoControls from './GizmoControls.svelte';
  import { type Gizmo, type GizmoHandlers, roundGizmoValue } from './gizmos';

  type Props = GizmoHandlers & { gizmo: Extract<Gizmo, { kind: 'rotate' }> };

  const { gizmo, onchange, onstart, onend }: Props = $props();

  let group = $state<Group>();

  // The ring turns the group, the angle is how far it turned since the drag started. It is
  // added up from small steps, so a drag can go past a full turn.
  const start = new Quaternion();
  let startAngle = 0;
  let turned = 0;
  let last = 0;

  function handleStart() {
    if (group) start.copy(group.quaternion);
    startAngle = gizmo.angle;
    turned = 0;
    last = 0;
    onstart?.();
  }

  function handleChange() {
    if (!group) return;
    const delta = group.quaternion.clone().multiply(start.clone().invert());
    const raw = 2 * Math.atan2([delta.x, delta.y, delta.z][gizmo.axis], delta.w);
    turned += angleStep(last, raw);
    last = raw;

    // the angle input goes from 0 to a full turn
    const full = Math.PI * 2;
    const angle = (((startAngle + turned) % full) + full) % full;
    onchange?.(gizmo.nodeId, { angle: roundGizmoValue(angle) });
  }
</script>

<!-- only the ring of the axis the node turns around -->
<GizmoControls
  bind:group
  mode="rotate"
  space="world"
  showX={gizmo.axis === 0}
  showY={gizmo.axis === 1}
  showZ={gizmo.axis === 2}
  position={gizmo.origin}
  onmouseDown={handleStart}
  onmouseUp={() => onend?.()}
  onobjectChange={handleChange}
/>
