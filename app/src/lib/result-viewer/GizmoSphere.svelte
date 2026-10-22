<script lang="ts">
  import { T } from '@threlte/core';
  import { GIZMO_STYLE } from './gizmo-style';
  import { gizmoTheme } from './gizmo-theme.svelte.ts';

  type Props = { opacity: number };

  const { opacity }: Props = $props();

  const { fill, wireframe } = GIZMO_STYLE.sphereSegments;
  const color = $derived(gizmoTheme.accent);
  const fillOpacity = $derived(gizmoTheme.opacity(opacity));
  const wireOpacity = $derived(gizmoTheme.opacity(opacity * GIZMO_STYLE.wireframeFactor));
</script>

<T.Mesh renderOrder={GIZMO_STYLE.renderOrder}>
  <T.SphereGeometry args={[1, ...fill]} />
  <T.MeshBasicMaterial
    {color}
    transparent
    opacity={fillOpacity}
    depthWrite={false}
    depthTest={false}
  />
</T.Mesh>
<T.Mesh renderOrder={GIZMO_STYLE.renderOrder}>
  <T.SphereGeometry args={[1, ...wireframe]} />
  <T.MeshBasicMaterial
    {color}
    wireframe
    transparent
    opacity={wireOpacity}
    depthWrite={false}
    depthTest={false}
  />
</T.Mesh>
