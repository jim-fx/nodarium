import type { Box } from '@nodarium/types';
import type { GraphManager } from '../graph-manager.svelte';
import type { GraphState } from '../graph-state.svelte';
import { distanceFromPointToSegment } from '../helpers';

export class EdgeInteractionManager {
  constructor(
    private graph: GraphManager,
    private state: GraphState
  ) {}

  private MIN_DISTANCE = 3;

  private _boundingBoxes = new Map<string, Box>();

  handleMouseDown() {
    this._boundingBoxes.clear();

    const possibleEdges = this.graph
      .getPossibleDropOnEdges(this.state.activeNodeId)
      .map(e => this.graph.getEdgeId(e));

    const edges = this.state.getEdges();
    for (const edge of edges) {
      const edgeId = edge[0];
      if (!possibleEdges.includes(edgeId)) {
        edges.delete(edgeId);
      }
    }

    for (const [edgeId, data] of edges) {
      const points = data.points;
      let minX = points[0].x + data.x1;
      let maxX = points[0].x + data.x1;
      let minY = points[0].z + data.y1;
      let maxY = points[0].z + data.y1;

      // Iterate through all points to find the true bounds
      for (let i = 0; i < points.length; i++) {
        const x = data.x1 + points[i].x;
        const y = data.y1 + points[i].z;
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }

      const boundingBox = {
        minX: minX - this.MIN_DISTANCE,
        maxX: maxX + this.MIN_DISTANCE,
        minY: minY - this.MIN_DISTANCE,
        maxY: maxY + this.MIN_DISTANCE
      };

      this._boundingBoxes.set(edgeId, boundingBox);
    }
  }

  handleMouseMove() {
    const [mouseX, mouseY] = this.state.mousePosition;
    const hoveredEdgeIds: string[] = [];

    const edges = this.state.getEdges();

    // Check if mouse is inside any bounding box
    for (const [edgeId, box] of this._boundingBoxes) {
      const isInside = mouseX >= box.minX
        && mouseX <= box.maxX
        && mouseY >= box.minY
        && mouseY <= box.maxY;

      if (isInside) {
        hoveredEdgeIds.push(edgeId);
      }
    }

    let hoveredEdgeId: string | null = null;
    let hoveredEdgeDistance = Infinity;

    const DENSITY = 10; // higher DENSITY = less points checked (yes density might not be the best name :-)
    for (const edgeId of hoveredEdgeIds) {
      const edge = edges.get(edgeId)!;
      for (let i = 0; i < edge.points.length - DENSITY; i += DENSITY) {
        const pointAx = edge.points[i].x + edge.x1;
        const pointAy = edge.points[i].z + edge.y1;
        const pointBx = edge.points[i + DENSITY].x + edge.x1;
        const pointBy = edge.points[i + DENSITY].z + edge.y1;
        const distance = distanceFromPointToSegment(
          pointAx,
          pointAy,
          pointBx,
          pointBy,
          mouseX,
          mouseY
        );
        if (distance < this.MIN_DISTANCE) {
          if (distance < hoveredEdgeDistance) {
            hoveredEdgeDistance = distance;
            hoveredEdgeId = edgeId;
          }
        }
      }
    }

    this.state.hoveredEdgeId = hoveredEdgeId;
  }

  handleMouseUp() {
    if (this.state.hoveredEdgeId) {
      const edge = this.graph.getEdgeById(this.state.hoveredEdgeId);
      if (edge) {
        this.graph.dropNodeOnEdge(this.state.activeNodeId, edge);
      }
      this.state.hoveredEdgeId = null;
    }
  }
}
