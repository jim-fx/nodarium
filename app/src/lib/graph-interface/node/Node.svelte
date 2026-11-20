<script lang="ts">
  import type { NodeInstance } from '@nodarium/types';
  import { T } from '@threlte/core';
  import { Color, LinearSRGBColorSpace, type Mesh } from 'three';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import { HIGHLIGHT_COLOR, WEBGL_NODE_MAX_ZOOM } from '../graph/constants';
  import { getNodeHeight, getNodeRows, headerSocketSide } from '../helpers/nodeHelpers';
  import { systemColor } from '../helpers/nodeSystem';
  import NodeFrag from './Node.frag';
  import NodeVert from './Node.vert';
  import NodeHtml from './NodeHTML.svelte';

  const graph = getGraphManager();
  const graphState = getGraphState();
  const colors = graphState.options.colors;
  const highlightColor = new Color().setStyle(HIGHLIGHT_COLOR, LinearSRGBColorSpace);

  type Props = {
    node: NodeInstance;
    inView: boolean;
  };
  let { node = $bindable(), inView }: Props = $props();

  const nodeType = $derived(node ? graph.getNodeType(node) : undefined);

  const isActive = $derived(graphState.activeNodeId === node.id);
  const isSelected = $derived(graphState.selectedNodes.has(node.id));
  const hasError = $derived(graphState.options.errorNodeId === node.id);
  const isHighlighted = $derived(graphState.isHighlighted(node.id));
  const strokeColor = $derived(
    graphState.options.theme
      && (isSelected
        ? colors.selected
        : isActive
        ? colors.active
        : isHighlighted
        ? highlightColor
        : hasError
        ? colors.error
        : colors.outline)
  );

  const rows = $derived(getNodeRows(node.type, nodeType).slice(0, 16));
  const sectionHeights = $derived(rows.map((row) => row.height));
  const socketMask = (side: 'left' | 'right') =>
    rows.reduce(
      (mask, row, i) => (row.socket === side ? mask | (1 << (i + 1)) : mask),
      side === 'right' && headerSocketSide(node.type, nodeType) ? 1 : 0
    );
  const leftSockets = $derived(socketMask('left'));
  const rightSockets = $derived(socketMask('right'));
  const possibleSockets = $derived(
    graphState.possibleSocketIds.size
      ? rows.reduce(
        (mask, row, i) =>
          graphState.possibleSocketIds.has(`${node.id}-${row.socketIndex}`)
            ? mask | (1 << (i + 1))
            : mask,
        0
      )
      : 0
  );

  const barHex = $derived(systemColor(node.type, graphState.options.systemColors));
  const barColor = $derived(new Color().setStyle(barHex ?? '#000', LinearSRGBColorSpace));

  let meshRef: Mesh | undefined = $state();

  const height = $derived(nodeType ? getNodeHeight(nodeType) : 20);

  const zoom = $derived(graphState.camera.position[2]);

  $effect(() => {
    if (meshRef && !node.state?.mesh) {
      node.state.mesh = meshRef;
      graphState.updateNodePosition(node);
    }
  });
</script>

<T.Mesh
  position.x={(node.state.x ?? node.position[0]) + 10}
  position.z={(node.state.y ?? node.position[1]) + height / 2}
  position.y={0.8}
  rotation.x={-Math.PI / 2}
  bind:ref={meshRef}
  visible={inView && zoom < WEBGL_NODE_MAX_ZOOM}
>
  <T.PlaneGeometry args={[20, height]} />
  <T.ShaderMaterial
    vertexShader={NodeVert}
    fragmentShader={NodeFrag}
    transparent
    uniforms={{
      uColorBright: { value: colors['layer-2'] },
      uColorDark: { value: colors['layer-1'] },
      uStrokeColor: { value: colors.outline.clone() },
      uBarColor: { value: new Color() },
      uHasBar: { value: false },
      uSectionHeights: { value: [5, 10] },
      uIsMuted: { value: false },
      uNumSections: { value: 2 },
      uLeftSockets: { value: 0 },
      uRightSockets: { value: 0 },
      uPossibleSockets: { value: 0 },
      uWidth: { value: 20 },
      uHeight: { value: 200 },
      uPixel: { value: 1 }
    }}
    uniforms.uIsMuted.value={node.muted}
    uniforms.uPixel.value={1 / zoom}
    uniforms.uHeight.value={height}
    uniforms.uSectionHeights.value={sectionHeights}
    uniforms.uNumSections.value={sectionHeights.length}
    uniforms.uLeftSockets.value={leftSockets}
    uniforms.uRightSockets.value={rightSockets}
    uniforms.uPossibleSockets.value={possibleSockets}
    uniforms.uBarColor.value={barColor}
    uniforms.uHasBar.value={!!barHex}
    uniforms.uStrokeColor.value={strokeColor}
  />
</T.Mesh>

<NodeHtml bind:node {inView} {isActive} {isSelected} {isHighlighted} {hasError} z={zoom} />
