import { animate, lerp } from '$lib/helpers';
import { type NodeInstance } from '@nodarium/types';
import type { GraphManager } from '../graph-manager.svelte';
import { type GraphState } from '../graph-state.svelte';
import { snapToGrid as snapPointToGrid } from '../helpers';
import { getNodeHeight } from '../helpers/nodeHelpers';
import { maxZoom, minZoom, zoomSpeed } from './constants';
import { EdgeInteractionManager } from './edge.events';

export class MouseEventManager {
  edgeInteractionManager: EdgeInteractionManager;

  constructor(
    private graph: GraphManager,
    private state: GraphState
  ) {
    this.edgeInteractionManager = new EdgeInteractionManager(graph, state);
  }

  handleWindowMouseUp(event: MouseEvent) {
    this.edgeInteractionManager.handleMouseUp();
    this.state.isPanning = false;
    if (!this.state.mouseDown) return;

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
      if (this.state.snapToGrid) {
        const snapLevel = this.state.getSnapLevel();
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
      this.graph.save();
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
      this.graph.save();
    } else if (this.state.activeSocket && event.ctrlKey) {
      // Handle automatic adding of nodes on ctrl+mouseUp
      this.state.edgeEndPosition = [
        this.state.mousePosition[0],
        this.state.mousePosition[1]
      ];

      if (typeof this.state.activeSocket.index === 'number') {
        this.state.addMenuPosition = [
          this.state.mousePosition[0],
          this.state.mousePosition[1] - 25 / this.state.cameraPosition[2]
        ];
      } else {
        this.state.addMenuPosition = [
          this.state.mousePosition[0] - 155 / this.state.cameraPosition[2],
          this.state.mousePosition[1] - 25 / this.state.cameraPosition[2]
        ];
      }
      return;
    }

    // check if camera moved
    if (
      clickedNodeId === -1
      && !this.state.boxSelection
      && this.state.cameraDown[0] === this.state.cameraPosition[0]
      && this.state.cameraDown[1] === this.state.cameraPosition[1]
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
  }

  handleContextMenu(event: MouseEvent) {
    if (!this.state.addMenuPosition) {
      event.preventDefault();
      this.state.openNodePalette();
    }
  }

  handleMouseDown(event: MouseEvent) {
    // Right click
    if (event.button === 2) {
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
    this.state.cameraDown[0] = this.state.cameraPosition[0];
    this.state.cameraDown[1] = this.state.cameraPosition[1];

    const clickedNodeId = this.state.getNodeIdFromEvent(event);
    this.state.mouseDownNodeId = clickedNodeId;

    // if we clicked on a node
    if (clickedNodeId !== -1) {
      if (this.state.activeNodeId === -1) {
        this.state.activeNodeId = clickedNodeId;
        // if the selected node is the same as the clicked node
      } else if (this.state.activeNodeId === clickedNodeId) {
        // $activeNodeId = -1;
        // if the clicked node is different from the selected node and secondary
      } else if (event.ctrlKey) {
        this.state.selectedNodes.add(this.state.activeNodeId);
        this.state.selectedNodes.delete(clickedNodeId);
        this.state.activeNodeId = clickedNodeId;
        // select the node
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

    if (event.ctrlKey && event.shiftKey && this.state.activeNodeId !== -1) {
      this.state.tryConnectToDebugNode(this.state.activeNodeId);
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

    this.state.mousePosition = this.state.projectScreenToWorld(mx, my);
    this.state.hoveredNodeId = this.state.getNodeIdFromEvent(event);

    if (!this.state.mouseDown) return;

    // we are creating a new edge here
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

    // handle box selection
    if (this.state.boxSelection) {
      event.preventDefault();
      event.stopPropagation();
      const mouseD = this.state.projectScreenToWorld(
        this.state.mouseDown[0],
        this.state.mouseDown[1]
      );
      const x1 = Math.min(mouseD[0], this.state.mousePosition[0]);
      const x2 = Math.max(mouseD[0], this.state.mousePosition[0]);
      const y1 = Math.min(mouseD[1], this.state.mousePosition[1]);
      const y2 = Math.max(mouseD[1], this.state.mousePosition[1]);
      for (const node of this.graph.nodes.values()) {
        if (!node?.state) continue;
        const x = node.position[0];
        const y = node.position[1];
        const height = getNodeHeight(node.state.type!);
        if (x > x1 - 20 && x < x2 && y > y1 - height && y < y2) {
          this.state.selectedNodes?.add(node.id);
        } else {
          this.state.selectedNodes?.delete(node.id);
        }
      }
      return;
    }

    // here we are handling dragging of nodes
    if (this.state.activeNodeId !== -1 && this.state.mouseDownNodeId !== -1) {
      this.edgeInteractionManager.handleMouseMove();
      const node = this.graph.getNode(this.state.activeNodeId);
      if (!node || event.buttons !== 1) return;

      node.state = node.state || {};

      const oldX = node.state.downX || 0;
      const oldY = node.state.downY || 0;

      let newX = oldX + (mx - this.state.mouseDown[0]) / this.state.cameraPosition[2];
      let newY = oldY + (my - this.state.mouseDown[1]) / this.state.cameraPosition[2];

      if (event.ctrlKey) {
        const snapLevel = this.state.getSnapLevel();
        if (this.state.snapToGrid) {
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

    // here we are handling panning of camera
    this.state.isPanning = true;
    const newX = this.state.cameraDown[0]
      - (mx - this.state.mouseDown[0]) / this.state.cameraPosition[2];
    const newY = this.state.cameraDown[1]
      - (my - this.state.mouseDown[1]) / this.state.cameraPosition[2];

    this.state.cameraPosition[0] = newX;
    this.state.cameraPosition[1] = newY;
  }

  handleMouseScroll(event: WheelEvent) {
    const bodyIsFocused = document.activeElement === document.body
      || document.activeElement === this.state.wrapper
      || document?.activeElement?.id === 'graph';
    if (!bodyIsFocused) return;

    // Define zoom speed and clamp it between -1 and 1
    const isNegative = event.deltaY < 0;
    const normalizedDelta = Math.abs(event.deltaY * 0.01);
    const delta = Math.pow(0.95, zoomSpeed * normalizedDelta);

    // Calculate new zoom level and clamp it between minZoom and maxZoom
    const newZoom = Math.max(
      minZoom,
      Math.min(
        maxZoom,
        isNegative
          ? this.state.cameraPosition[2] / delta
          : this.state.cameraPosition[2] * delta
      )
    );

    // Calculate the ratio of the new zoom to the original zoom
    const zoomRatio = newZoom / this.state.cameraPosition[2];

    // Update camera position and zoom level
    this.state.cameraPosition[0] = this.state.mousePosition[0]
      - (this.state.mousePosition[0] - this.state.cameraPosition[0])
        / zoomRatio;
    this.state.cameraPosition[1] = this.state.mousePosition[1]
      - (this.state.mousePosition[1] - this.state.cameraPosition[1])
        / zoomRatio;
    this.state.cameraPosition[2] = newZoom;
  }
}
