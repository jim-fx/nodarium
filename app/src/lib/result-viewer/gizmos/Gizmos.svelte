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

  // A drag can lose its end: the gizmo unmounts when the active node changes, or the pointer is
  // released outside of the canvas. Every drag that started ends exactly once.
  let dragging = false;
  function start() {
    dragging = true;
    onstart?.();
  }
  function end() {
    if (!dragging) return;
    dragging = false;
    onend?.();
  }

  // a number, so a new gizmo of the same node on every tick of the drag doesn't end it
  const gizmoNodeId = $derived(gizmo?.nodeId);
  $effect(() => {
    void gizmoNodeId;
    return end;
  });

  const handlers = $derived({ onchange, onstart: start, onend: end });
</script>

<svelte:window onpointerup={end} onpointercancel={end} onblur={end} />

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
