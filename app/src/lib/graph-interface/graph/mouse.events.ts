import { type NodeInstance } from '@nodarium/types';
import type { GraphManager } from '../graph-manager.svelte';
import { type GraphState } from '../graph-state.svelte';
import { animate, lerp, snapToGrid as snapPointToGrid } from '../helpers/index.ts';
import { boxesOverlap, nodeBox } from '../helpers/nodeBox';
import { EdgeInteractionManager } from './edge.events';

export class MouseEventManager {
  edgeInteractionManager: EdgeInteractionManager;
  private pendingSelectionFrame = false;

  constructor(
    private graph: GraphManager,
    private state: GraphState
  ) {
    this.edgeInteractionManager = new EdgeInteractionManager(graph, state);
  }

  handleWindowMouseUp(event: MouseEvent) {
    const droppedOnEdge = this.edgeInteractionManager.handleMouseUp();
    this.state.isPanning = false;
    if (!this.state.mouseDown) return;

    const clickedSocket = this.state.releasedSocket(event);
    const droppedOnSocket = !!this.state.hoveredSocket;

    const activeNode = this.graph.getNode(this.state.activeNodeId);

    const clickedNodeId = this.state.getNodeIdFromEvent(event);

    if (clickedNodeId !== -1) {
      if (activeNode) {
        if (!activeNode?.state?.isMoving && !event.ctrlKey && !event.shiftKey) {
          this.state.activeNodeId = clickedNodeId;
          this.state.clearSelection();
        }
      }
    }

    if (activeNode?.state?.isMoving) {
      activeNode.state = activeNode.state || {};
      activeNode.state.isMoving = false;
      if (this.state.options.snapToGrid) {
        const snapLevel = this.state.camera.snapLevel();
        activeNode.position[0] = snapPointToGrid(
          activeNode?.state?.x ?? activeNode.position[0],
          5 / snapLevel
        );
        activeNode.position[1] = snapPointToGrid(
          activeNode?.state?.y ?? activeNode.position[1],
          5 / snapLevel
        );
      } else {
        activeNode.position[0] = activeNode?.state?.x ?? activeNode.position[0];
        activeNode.position[1] = activeNode?.state?.y ?? activeNode.position[1];
      }
      const nodes = [
        ...[...(this.state.selectedNodes?.values() || [])].map((id) => this.graph.getNode(id))
      ] as NodeInstance[];

      const vec = [
        activeNode.position[0] - (activeNode?.state.x || 0),
        activeNode.position[1] - (activeNode?.state.y || 0)
      ];

      for (const node of nodes) {
        if (!node) continue;
        node.state = node.state || {};
        const { x, y } = node.state;
        if (x !== undefined && y !== undefined) {
          node.position[0] = x + vec[0];
          node.position[1] = y + vec[1];
        }
      }
      nodes.push(activeNode);

      if (droppedOnEdge) {
        const moved = this.graph.dropNodeOnEdge(activeNode.id, droppedOnEdge);
        for (const [node, x] of moved) {
          node.state.x ??= x;
          node.state.y ??= node.position[1];
          if (!nodes.includes(node)) nodes.push(node);
        }
      }

      animate(500, (a: number) => {
        for (const node of nodes) {
          if (
            node?.state
            && node.state['x'] !== undefined
            && node.state['y'] !== undefined
          ) {
            node.state.x = lerp(node.state.x, node.position[0], a);
            node.state.y = lerp(node.state.y, node.position[1], a);
            this.state.updateNodePosition(node);
            if (node?.state?.isMoving) {
              return false;
            }
          }
        }
      });
      this.graph.commit({ build: false });
    } else if (this.state.hoveredSocket && this.state.activeSocket) {
      if (
        typeof this.state.hoveredSocket.index === 'number'
        && typeof this.state.activeSocket.index === 'string'
      ) {
        this.graph.createEdge(
          this.state.hoveredSocket.node,
          this.state.hoveredSocket.index || 0,
          this.state.activeSocket.node,
          this.state.activeSocket.index
        );
      } else if (
        typeof this.state.activeSocket.index == 'number'
        && typeof this.state.hoveredSocket.index === 'string'
      ) {
        this.graph.createEdge(
          this.state.activeSocket.node,
          this.state.activeSocket.index || 0,
          this.state.hoveredSocket.node,
          this.state.hoveredSocket.index
        );
      }
    } else if (this.state.activeSocket && event.ctrlKey) {
      // Handle automatic adding of nodes on ctrl+mouseUp
      this.state.edgeEndPosition = [
        this.state.mousePosition[0],
        this.state.mousePosition[1]
      ];

      if (typeof this.state.activeSocket.index === 'number') {
        this.state.addMenuPosition = [
          this.state.mousePosition[0],
          this.state.mousePosition[1] - 25 / this.state.camera.position[2]
        ];
      } else {
        this.state.addMenuPosition = [
          this.state.mousePosition[0] - 155 / this.state.camera.position[2],
          this.state.mousePosition[1] - 25 / this.state.camera.position[2]
        ];
      }
      return;
    }

    if (
      clickedNodeId === -1
      && !this.state.boxSelection
      && this.state.cameraDown[0] === this.state.camera.position[0]
      && this.state.cameraDown[1] === this.state.camera.position[1]
      && this.state.isBodyFocused()
    ) {
      this.state.activeNodeId = -1;
      this.state.clearSelection();
    }

    this.state.mouseDown = null;
    this.state.boxSelection = false;
    this.state.activeSocket = null;
    this.state.possibleSockets = [];
    this.state.hoveredSocket = null;
    this.state.addMenuPosition = null;

    if (clickedSocket && !droppedOnSocket) this.state.pickSocket(clickedSocket);
  }

  /** A press anywhere but on a socket leaves the state of a picked socket */
  handleWindowMouseDown() {
    this.state.unpickSocket();
  }

  private connectToDebugNode(nodeId: number) {
    if (nodeId === -1) return;
    const debugNode = this.graph.tryConnectToDebugNode(nodeId);
    if (debugNode) this.state.updateNodePosition(debugNode);
  }

  handleContextMenu(event: MouseEvent) {
    // shift right click on a node attaches the debug node instead
    if (event.shiftKey && this.state.getNodeIdFromEvent(event) !== -1) {
      event.preventDefault();
      return;
    }
    if (!this.state.addMenuPosition) {
      event.preventDefault();
      this.state.openNodePalette();
    }
  }

  handleMouseDown(event: MouseEvent) {
    if (event.button === 2) {
      if (event.shiftKey) this.connectToDebugNode(this.state.getNodeIdFromEvent(event));
      return;
    }

    if (this.state.mouseDown) return;
    this.state.edgeEndPosition = null;
    const target = event.target as HTMLElement;

    if (
      target.nodeName !== 'CANVAS'
      && !target.classList.contains('node')
      && !target.classList.contains('content')
    ) {
      return;
    }

    const mx = event.clientX - this.state.rect.x;
    const my = event.clientY - this.state.rect.y;

    this.state.mouseDown = [mx, my];
    this.state.cameraDown[0] = this.state.camera.position[0];
    this.state.cameraDown[1] = this.state.camera.position[1];

    const clickedNodeId = this.state.getNodeIdFromEvent(event);
    this.state.mouseDownNodeId = clickedNodeId;

    if (clickedNodeId !== -1) {
      if (event.ctrlKey && event.shiftKey) {
        this.connectToDebugNode(clickedNodeId);
        return;
      }
      if (this.state.activeNodeId === -1) {
        this.state.activeNodeId = clickedNodeId;
      } else if (this.state.activeNodeId === clickedNodeId) {
        // clicking the active node again keeps the selection as it is
      } else if (event.ctrlKey) {
        this.state.selectedNodes.add(this.state.activeNodeId);
        this.state.selectedNodes.delete(clickedNodeId);
        this.state.activeNodeId = clickedNodeId;
      } else if (event.shiftKey) {
        const activeNode = this.graph.getNode(this.state.activeNodeId);
        const newNode = this.graph.getNode(clickedNodeId);
        if (activeNode && newNode) {
          const edge = this.graph.getNodesBetween(activeNode, newNode);
          if (edge) {
            this.state.selectedNodes.clear();
            for (const node of edge) {
              this.state.selectedNodes.add(node.id);
            }
            this.state.selectedNodes.add(clickedNodeId);
          }
        }
      } else if (!this.state.selectedNodes.has(clickedNodeId)) {
        this.state.activeNodeId = clickedNodeId;
        this.state.clearSelection();
      }
      this.edgeInteractionManager.handleMouseDown();
    } else if (event.ctrlKey) {
      this.state.boxSelection = true;
    }

    const node = this.graph.getNode(this.state.activeNodeId);
    if (!node) return;
    node.state = node.state || {};
    node.state.downX = node.position[0];
    node.state.downY = node.position[1];

    if (this.state.selectedNodes) {
      for (const nodeId of this.state.selectedNodes) {
        const n = this.graph.getNode(nodeId);
        if (!n) continue;
        n.state = n.state || {};
        n.state.downX = n.position[0];
        n.state.downY = n.position[1];
      }
    }

    this.state.edgeEndPosition = null;
  }

  handleWindowMouseMove(event: MouseEvent) {
    const mx = event.clientX - this.state.rect.x;
    const my = event.clientY - this.state.rect.y;

    this.state.mousePosition = this.state.camera.toWorld(mx, my);
    this.state.hoveredNodeId = this.state.getNodeIdFromEvent(event);
    this.state.movePointer(event);

    if (!this.state.mouseDown) return;

    if (this.state.activeSocket || this.state.possibleSockets?.length) {
      let smallestDist = 1000;
      let _socket;
      for (const socket of this.state.possibleSockets) {
        const dist = Math.sqrt(
          (socket.position[0] - this.state.mousePosition[0]) ** 2
            + (socket.position[1] - this.state.mousePosition[1]) ** 2
        );
        if (dist < smallestDist) {
          smallestDist = dist;
          _socket = socket;
        }
      }

      if (_socket && smallestDist < 1.5) {
        this.state.mousePosition = _socket.position;
        this.state.hoveredSocket = _socket;
      } else {
        this.state.hoveredSocket = null;
      }
      return;
    }

    if (this.state.boxSelection) {
      event.preventDefault();
      event.stopPropagation();
      if (!this.pendingSelectionFrame) {
        this.pendingSelectionFrame = true;
        requestAnimationFrame(() => {
          this.pendingSelectionFrame = false;
          if (!this.state.mouseDown) return;
          const mouseD = this.state.camera.toWorld(
            this.state.mouseDown[0],
            this.state.mouseDown[1]
          );
          const selection = {
            minX: Math.min(mouseD[0], this.state.mousePosition[0]),
            maxX: Math.max(mouseD[0], this.state.mousePosition[0]),
            minY: Math.min(mouseD[1], this.state.mousePosition[1]),
            maxY: Math.max(mouseD[1], this.state.mousePosition[1])
          };
          for (const node of this.graph.nodes.values()) {
            if (!node?.state) continue;
            if (boxesOverlap(nodeBox(node, this.graph.getNodeType(node)), selection)) {
              this.state.selectedNodes?.add(node.id);
            } else {
              this.state.selectedNodes?.delete(node.id);
            }
          }
        });
      }
      return;
    }

    if (this.state.activeNodeId !== -1 && this.state.mouseDownNodeId !== -1) {
      this.edgeInteractionManager.handleMouseMove();
      const node = this.graph.getNode(this.state.activeNodeId);
      if (!node || event.buttons !== 1) return;

      node.state = node.state || {};

      const oldX = node.state.downX || 0;
      const oldY = node.state.downY || 0;

      let newX = oldX + (mx - this.state.mouseDown[0]) / this.state.camera.position[2];
      let newY = oldY + (my - this.state.mouseDown[1]) / this.state.camera.position[2];

      if (event.ctrlKey) {
        const snapLevel = this.state.camera.snapLevel();
        if (this.state.options.snapToGrid) {
          newX = snapPointToGrid(newX, 5 / snapLevel);
          newY = snapPointToGrid(newY, 5 / snapLevel);
        }
      }

      if (!node.state.isMoving) {
        const dist = Math.sqrt((oldX - newX) ** 2 + (oldY - newY) ** 2);
        if (dist > 0.2) {
          node.state.isMoving = true;
        }
      }

      const vecX = oldX - newX;
      const vecY = oldY - newY;

      if (this.state.selectedNodes?.size) {
        for (const nodeId of this.state.selectedNodes) {
          const n = this.graph.getNode(nodeId);
          if (!n?.state) continue;
          n.state.x = (n?.state?.downX || 0) - vecX;
          n.state.y = (n?.state?.downY || 0) - vecY;
          this.state.updateNodePosition(n);
        }
      }

      node.state.x = newX;
      node.state.y = newY;

      this.state.updateNodePosition(node);

      return;
    }

    this.state.isPanning = true;
    this.state.camera.pan(
      this.state.cameraDown,
      mx - this.state.mouseDown[0],
      my - this.state.mouseDown[1]
    );
  }

  handleMouseScroll(event: WheelEvent) {
    const bodyIsFocused = document.activeElement === document.body
      || document.activeElement === this.state.wrapper
      || document?.activeElement?.id === 'graph';
    if (!bodyIsFocused) return;

    this.state.camera.zoomAt(
      [this.state.mousePosition[0], this.state.mousePosition[1]],
      event.deltaY
    );
  }
}
