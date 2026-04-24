<script lang="ts">
  import { appSettings } from '$lib/settings/app-settings.svelte';
  import type { NodeInstance } from '@nodarium/types';
  import { T } from '@threlte/core';
  import { type Mesh } from 'three';
  import { getGraphState } from '../graph-state.svelte';
  import { colors } from '../graph/colors.svelte';
  import { getNodeHeight, getParameterHeight } from '../helpers/nodeHelpers';
  import NodeFrag from './Node.frag';
  import NodeVert from './Node.vert';
  import NodeHtml from './NodeHTML.svelte';

  const graphState = getGraphState();

  type Props = {
    node: NodeInstance;
    inView: boolean;
  };
  let { node = $bindable(), inView }: Props = $props();

  const nodeType = $derived(node.state.type!);

  const isActive = $derived(graphState.activeNodeId === node.id);
  const isSelected = $derived(graphState.selectedNodes.has(node.id));
  const strokeColor = $derived(
    appSettings.value.theme
      && (isSelected
        ? colors.selected
        : isActive
        ? colors.active
        : colors.outline)
  );

  const sectionHeights = $derived(
    Object
      .keys(nodeType.inputs || {})
      .map(key => getParameterHeight(nodeType, key) / 10)
      .filter(b => !!b)
  );

  let meshRef: Mesh | undefined = $state();

  const height = $derived(getNodeHeight(node.state.type!));

  const zoom = $derived(graphState.cameraPosition[2]);

  $effect(() => {
    if (meshRef && !node.state?.mesh) {
      node.state.mesh = meshRef;
      graphState.updateNodePosition(node);
    }
  });
  const zoomValue = $derived(
    (Math.log(graphState.cameraPosition[2]) - Math.log(1)) / (Math.log(40) - Math.log(1))
  );
  // const zoomValue = (graphState.cameraPosition[2] - 1) / 39;
</script>

<T.Mesh
  position.x={(node.state.x ?? node.position[0]) + 10}
  position.z={(node.state.y ?? node.position[1]) + height / 2}
  position.y={0.8}
  rotation.x={-Math.PI / 2}
  bind:ref={meshRef}
  visible={inView && zoom < 7}
>
  <T.PlaneGeometry args={[20, height]} radius={1} />
  <T.ShaderMaterial
    vertexShader={NodeVert}
    fragmentShader={NodeFrag}
    transparent
    uniforms={{
      uColorBright: { value: colors['layer-2'] },
      uColorDark: { value: colors['layer-1'] },
      uStrokeColor: { value: colors.outline.clone() },
      uSectionHeights: { value: [5, 10] },
      uNumSections: { value: 2 },
      uWidth: { value: 20 },
      uHeight: { value: 200 },
      uZoom: { value: 1.0 }
    }}
    uniforms.uZoom.value={zoomValue}
    uniforms.uHeight.value={height}
    uniforms.uSectionHeights.value={sectionHeights}
    uniforms.uNumSections.value={sectionHeights.length}
    uniforms.uStrokeColor.value={strokeColor}
  />
</T.Mesh>

<NodeHtml bind:node {inView} {isActive} {isSelected} z={zoom} />
