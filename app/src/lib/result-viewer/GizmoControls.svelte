<script lang="ts">
  import { TransformControls } from '@threlte/extras';
  import type { ComponentProps } from 'svelte';
  import type { TransformControls as Controls } from 'three/examples/jsm/controls/TransformControls.js';
  import { GIZMO_STYLE } from './gizmo-style';
  import { gizmoTheme } from './gizmo-theme.svelte.ts';

  // the transform controls of every gizmo, with the look of the app
  let { group = $bindable(), ...props }: ComponentProps<typeof TransformControls> = $props();

  let controls = $state<Controls>();

  $effect(() => {
    // the handles all share one color, the one under the mouse gets the active color
    const { handle, hover } = gizmoTheme;
    controls?.setColors(handle, handle, handle, hover);
  });
</script>

<TransformControls bind:controls bind:group size={GIZMO_STYLE.handleSize} {...props} />
