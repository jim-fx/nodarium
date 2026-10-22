<script lang="ts">
  import BranchGizmo from './BranchGizmo.svelte';
  import type { Gizmo, GizmoChange, GizmoMode } from './gizmos';
  import PositionGizmo from './PositionGizmo.svelte';
  import RotateGizmo from './RotateGizmo.svelte';
  import StemGizmo from './StemGizmo.svelte';

  type Props = {
    gizmos: Gizmo[];
    mode: GizmoMode;
    onchange?: (nodeId: number, change: GizmoChange) => void;
    onstart?: () => void;
    onend?: () => void;
  };

  const { gizmos, mode, onchange, onstart, onend }: Props = $props();

  // only the active node has a gizmo
  const gizmo = $derived(gizmos[0]);
  const handlers = $derived({ onchange, onstart, onend });
</script>

{#if gizmo}
  {#key gizmo.nodeId}
    {#if gizmo.kind === 'force' || gizmo.kind === 'position'}
      <PositionGizmo {gizmo} {mode} {...handlers} />
    {:else if gizmo.kind === 'stem'}
      <StemGizmo {gizmo} {...handlers} />
    {:else if gizmo.kind === 'rotate'}
      <RotateGizmo {gizmo} {...handlers} />
    {:else if gizmo.kind === 'branch'}
      <BranchGizmo {gizmo} {...handlers} />
    {/if}
  {/key}
{/if}
