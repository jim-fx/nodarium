<script lang="ts">
  import { GROUP_INPUT } from '@nodarium/types';
  import { T } from '@threlte/core';
  import { Color, LinearSRGBColorSpace } from 'three';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import NodeVert from './Node.vert';
  import SocketHighlightFrag from './SocketHighlight.frag';

  // The circles NodeHeader, NodeParameter and NodeOutput show on the sockets the dragged edge
  // fits, for the zoom where the nodes are drawn by WebGL
  const graph = getGraphManager();
  const graphState = getGraphState();

  const socket = $derived(graphState.connectingSocket);
  const color = $derived(
    new Color().setStyle(
      graphState.colors.getColor(
        (socket && graph.getSocketType(socket.node, socket.index)) || 'unknown'
      ),
      LinearSRGBColorSpace
    )
  );
</script>

{#each graphState.possibleSockets as s (`${s.node.id}-${s.index}`)}
  <T.Mesh
    position.x={s.position[0]}
    position.z={s.position[1]}
    position.y={1}
    rotation.x={-Math.PI / 2}
  >
    <T.PlaneGeometry args={[5, 5]} />
    <T.ShaderMaterial
      vertexShader={NodeVert}
      fragmentShader={SocketHighlightFrag}
      transparent
      depthWrite={false}
      uniforms={{ uColor: { value: new Color() }, uOpacity: { value: 0.5 } }}
      uniforms.uColor.value={color}
      uniforms.uOpacity.value={s.index === 0 && s.node.type !== GROUP_INPUT ? 0.7 : 0.5}
    />
  </T.Mesh>
{/each}
