<script lang="ts">
  import type { Frame } from '@nodarium/types';
  import { T } from '@threlte/core';
  import { Color } from 'three';
  import { getGraphManager, getGraphState } from '../graph-state.svelte';
  import { FRAME_HEADER, frameBox } from '../helpers/frames';
  import { boxesOverlap, nodeBox } from '../helpers/nodeBox';
  import NodeVert from '../node/Node.vert';
  import FrameFrag from './Frame.frag';

  const { frame }: { frame: Frame } = $props();

  const graph = getGraphManager();
  const graphState = getGraphState();
  const colors = graphState.options.colors;

  // the title is 28 px tall with its padding, it fits into the header from this zoom on
  const LABEL_INSIDE_ZOOM = 28 / FRAME_HEADER;

  const box = $derived(
    frameBox(
      frame.nodes.flatMap((id) => {
        const node = graph.getNode(id);
        return node ? [nodeBox(node, graph.getNodeType(node))] : [];
      })
    )
  );
  const inView = $derived(!!box && boxesOverlap(box, graphState.camera.bounds));
  const zoom = $derived(graphState.camera.position[2]);
  const isSelected = $derived(graphState.activeFrameId === frame.id);
  const isRenaming = $derived(graphState.renamingFrameId === frame.id);

  const fillColor = $derived(
    graphState.options.theme && colors[`frame-${frame.color ?? 'gray'}`]
  );
  const strokeColor = $derived(
    graphState.options.theme && (isSelected ? colors.selected : fillColor)
  );

  function finishRename(input: HTMLInputElement, keep: boolean) {
    if (graphState.renamingFrameId !== frame.id) return;
    graphState.renamingFrameId = -1;
    const label = input.value.trim();
    if (keep && label) graph.renameFrame(frame.id, label);
  }

  function focus(input: HTMLInputElement) {
    input.focus();
    input.select();
  }
</script>

{#if box}
  <T.Mesh
    position.x={(box.minX + box.maxX) / 2}
    position.z={(box.minY + box.maxY) / 2}
    position.y={0}
    rotation.x={-Math.PI / 2}
    scale.x={box.maxX - box.minX}
    scale.y={box.maxY - box.minY}
    visible={inView}
  >
    <T.PlaneGeometry args={[1, 1]} />
    <T.ShaderMaterial
      vertexShader={NodeVert}
      fragmentShader={FrameFrag}
      transparent
      depthWrite={false}
      uniforms={{
        uSize: { value: [1, 1] },
        uPixel: { value: 1 },
        uColor: { value: new Color() },
        uStrokeColor: { value: new Color() },
        uStrokeAlpha: { value: 0.45 },
        uStrokeWidth: { value: 1 }
      }}
      uniforms.uSize.value={[box.maxX - box.minX, box.maxY - box.minY]}
      uniforms.uPixel.value={1 / zoom}
      uniforms.uColor.value={fillColor}
      uniforms.uStrokeColor.value={strokeColor}
      uniforms.uStrokeAlpha.value={isSelected ? 1 : 0.45}
      uniforms.uStrokeWidth.value={isSelected ? 2 : 1}
    />
  </T.Mesh>

  <div
    class="frame-header"
    class:outside={zoom < LABEL_INSIDE_ZOOM}
    class:hidden={!inView}
    data-frame-id={frame.id}
    role="group"
    aria-label={frame.label}
    style:left="{box.minX * 10}px"
    style:top="{box.minY * 10}px"
    style:width="{(box.maxX - box.minX) * 10}px"
    style:height="{FRAME_HEADER * 10}px"
    style:--label-scale={10 / zoom}
  >
    {#if isRenaming}
      <input
        class="frame-label"
        value={frame.label}
        aria-label="Frame title"
        use:focus
        onkeydown={(ev) => {
          if (ev.key === 'Enter') finishRename(ev.currentTarget, true);
          else if (ev.key === 'Escape') finishRename(ev.currentTarget, false);
          ev.stopPropagation();
        }}
        onblur={(ev) => finishRename(ev.currentTarget, true)}
      />
    {:else}
      <span
        class="frame-label"
        role="heading"
        aria-level={3}
        ondblclick={() => (graphState.renamingFrameId = frame.id)}
      >{frame.label}</span>
    {/if}
  </div>
{/if}

<style>
  .frame-header {
    position: absolute;
    box-sizing: border-box;
  }

  .frame-header.hidden {
    display: none;
  }

  .frame-label {
    position: absolute;
    left: 0;
    top: 0;
    transform: scale(var(--label-scale));
    transform-origin: top left;
    padding: 4px 8px;
    font-family: var(--font-family);
    font-size: 14px;
    font-weight: 600;
    line-height: 20px;
    color: var(--color-text);
    white-space: nowrap;
    user-select: none;
  }

  .outside .frame-label {
    top: auto;
    bottom: 100%;
    transform-origin: bottom left;
    padding: 0 0 2px 0;
  }

  input.frame-label {
    width: 240px;
    margin: 0;
    border: none;
    outline: 1px solid var(--color-active);
    border-radius: 2px;
    background: var(--color-layer-1);
    user-select: text;
  }
</style>
