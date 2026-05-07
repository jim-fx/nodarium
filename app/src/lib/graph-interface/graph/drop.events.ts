import { toast } from '@nodarium/ui';
import { GraphSchema, type NodeId } from '@nodarium/types';
import type { GraphManager } from '../graph-manager.svelte';
import type { GraphState } from '../graph-state.svelte';

export class FileDropEventManager {
  constructor(
    private graph: GraphManager,
    private state: GraphState
  ) {}

  handleFileDrop(event: DragEvent) {
    event.preventDefault();
    this.state.isDragging = false;
    if (!event.dataTransfer) return;
    const nodeId = event.dataTransfer.getData('data/node-id') as NodeId;
    let mx = event.clientX - this.state.rect.x;
    let my = event.clientY - this.state.rect.y;

    if (nodeId) {
      const nodeOffsetX = event.dataTransfer.getData('data/node-offset-x');
      const nodeOffsetY = event.dataTransfer.getData('data/node-offset-y');
      if (nodeOffsetX && nodeOffsetY) {
        mx += parseInt(nodeOffsetX);
        my += parseInt(nodeOffsetY);
      }

      let props = {};
      const rawNodeProps = event.dataTransfer.getData('data/node-props');
      if (rawNodeProps) {
        try {
          props = JSON.parse(rawNodeProps);
        } catch (e) {
          console.error('Failed to parse node dropped', e);
        }
      }

      const pos = this.state.projectScreenToWorld(mx, my);
      this.graph.registry.load([nodeId]).then(() => {
        this.graph.createNode({
          type: nodeId,
          props,
          position: pos
        });
      }).catch((e) => {
        toast(`Failed to load node: ${nodeId}`, 'error');
        console.error(e);
      });
    } else if (event.dataTransfer.files.length) {
      const file = event.dataTransfer.files[0];

      if (file.type === 'application/wasm') {
        const reader = new FileReader();
        reader.onload = async (e) => {
          const buffer = e.target?.result;
          if (buffer?.constructor === ArrayBuffer) {
            const nodeType = await this.graph.registry.register(nodeId, buffer);

            this.graph.createNode({
              type: nodeType.id,
              props: {},
              position: this.state.projectScreenToWorld(mx, my)
            });
          }
        };
        reader.readAsArrayBuffer(file);
      } else if (file.type === 'application/json') {
        const reader = new FileReader();
        reader.onload = (e) => {
          const buffer = e.target?.result as ArrayBuffer;
          if (buffer) {
            try {
              const state = GraphSchema.parse(JSON.parse(buffer.toString()));
              this.graph.load(state);
            } catch (e) {
              toast('Failed to load graph: invalid file', 'error');
              console.error(e);
            }
          }
        };
        reader.readAsText(file);
      }
    }
  }

  handleMouseLeave() {
    this.state.isDragging = false;
    this.state.isPanning = false;
  }

  handleDragEnter(e: DragEvent) {
    e.preventDefault();
    this.state.isDragging = true;
    this.state.isPanning = false;
  }

  handleDragOver(e: DragEvent) {
    e.preventDefault();
    this.state.isDragging = true;
    this.state.isPanning = false;
  }

  handleDragEnd(e: DragEvent) {
    e.preventDefault();
    this.state.isDragging = true;
    this.state.isPanning = false;
  }

  getEventListenerProps() {
    return {
      ondragenter: (ev: DragEvent) => this.handleDragEnter(ev),
      ondragover: (ev: DragEvent) => this.handleDragOver(ev),
      ondragexit: (ev: DragEvent) => this.handleDragEnd(ev),
      ondrop: (ev: DragEvent) => this.handleFileDrop(ev),
      onmouseleave: () => this.handleMouseLeave()
    };
  }
}
