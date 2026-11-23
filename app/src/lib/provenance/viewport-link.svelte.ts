import type { PartHighlight } from '#lib/result-viewer/provenance/highlight.ts';
import type { PartHit } from '#lib/result-viewer/provenance/part-hit.ts';
import type { GraphManager, GraphState } from '@nodarium/graph-interface';
import { NO_PATH } from '@nodarium/utils';
import {
  buildIndex,
  composeHighlight,
  downstream,
  isEmpty,
  isInstanceNode,
  lineageOf,
  nodeOfPart,
  type NodeTraits,
  type Parts,
  partsOfNodes,
  partsOfPicked,
  partsOfRunning,
  type PickedParts,
  type ResultParts,
  visibleNode
} from './provenance';

type Sources = {
  enabled: () => boolean;
  manager: () => GraphManager | undefined;
  graphState: () => GraphState | undefined;
  /** what the result the viewer shows knows of its parts */
  result: () => ResultParts;
  traits: NodeTraits;
  notify: (message: string) => void;
};

/** What was clicked in the viewer: the node it led to and the parts to point out among its own */
type Picked = PickedParts & { nodeId: number };

/** What the local view shows, kept as nodes and parts so it survives a new result */
type LocalView = {
  /** the graph the nodes are of, another project has other nodes under the same ids */
  manager: GraphManager;
  graphId: number;
  entered: number[];
  nodeIds: number[];
  picked?: Picked;
};

/**
 * Links the parts of the result in the viewer to the nodes in the editor, both ways. Nothing is
 * computed while it is off, or while nothing is selected or hovered.
 */
export class ViewportLink {
  /** bumped with every result the viewer shows */
  resultVersion = $state(0);

  private hoveredPart = $state.raw<PartHit>();
  private hoveredNode = $state<number>();
  private picked = $state.raw<Picked>();
  private requestedLocal = $state.raw<LocalView>();

  constructor(private sources: Sources) {}

  private enabled = $derived.by(() => this.sources.enabled());

  private index = $derived.by(() => {
    const manager = this.sources.manager();
    if (!manager || manager.status !== 'idle') return;
    return buildIndex(manager.serialize(), this.sources.traits);
  });

  private result = $derived.by(() => {
    void this.resultVersion;
    return this.sources.result();
  });

  private entered = $derived.by(() =>
    this.sources.manager()?.path.map((entry) => entry.nodeId) ?? []
  );

  private selected = $derived.by(() => {
    const state = this.sources.graphState();
    if (!state) return [];
    const ids = [...state.selectedNodes];
    if (state.activeNodeId !== -1 && !ids.includes(state.activeNodeId)) {
      ids.push(state.activeNodeId);
    }
    return ids;
  });

  private activePicked = $derived.by(() =>
    this.picked && this.picked.nodeId === this.sources.graphState()?.activeNodeId
      ? this.picked
      : undefined
  );

  private partsOfNodes(nodeIds: number[], entered: number[]): Parts | undefined {
    const index = this.index;
    return index && partsOfNodes(index, this.result, entered, nodeIds);
  }

  /** The node that runs and made the hovered part */
  private hoveredCreator = $derived.by(() => {
    const hit = this.hoveredPart;
    // the index is only read, and with that built, once there is something to look up
    if (!this.enabled || !hit) return;
    const index = this.index;
    return index && nodeOfPart(index, this.result, hit);
  });

  private hoveredParts(): Parts | undefined {
    const index = this.index;
    if (!index) return;
    const creator = this.hoveredCreator;
    if (this.hoveredPart && creator !== undefined) {
      // the parts of a selected node are tinted already, the hover would hide a click on them
      const visible = visibleNode(index.chains.get(creator) ?? [], this.entered);
      if (!this.local && visible !== undefined && this.selected.includes(visible)) return;
      return partsOfRunning(index, this.result, [creator]);
    }
    const node = this.hoveredNode;
    if (node !== undefined && !this.selected.includes(node)) {
      return this.partsOfNodes([node], this.entered);
    }
  }

  private isolated = $derived.by(() => {
    const local = this.requestedLocal;
    const manager = this.sources.manager();
    if (!this.enabled || !local || local.manager !== manager || local.graphId !== manager.id) {
      return;
    }
    const parts = local.picked
      ? partsOfPicked(this.result.links, local.picked, true)
      : this.partsOfNodes(local.nodeIds, local.entered);
    // a node that was removed, or makes nothing any more, would leave the viewer empty
    return parts && !isEmpty(parts) ? parts : undefined;
  });

  private local = $derived.by(() => (this.isolated ? this.requestedLocal : undefined));

  /** Whether the viewer only shows the parts of the selection */
  localView = $derived.by(() => !!this.local);

  /** What the viewer tints or hides, `undefined` while there is nothing to point out */
  highlight: PartHighlight | undefined = $derived.by(() => {
    if (!this.enabled) return;
    const local = this.local;
    const selected = this.selected;
    const hovering = !!this.hoveredPart || this.hoveredNode !== undefined;
    if (!local && !selected.length && !hovering) return;
    if (!this.index) return;

    const isolated = this.isolated;
    const picked = this.activePicked;
    return composeHighlight({
      selected: isolated ? undefined : this.partsOfNodes(selected, this.entered),
      picked: picked && !isolated ? partsOfPicked(this.result.links, picked) : undefined,
      hovered: this.hoveredParts(),
      isolated,
      selection: `${this.entered.join()}:${selected.join()}:${picked?.paths.join() ?? ''}:${
        picked?.creators.join() ?? ''
      }`
    });
  });

  /** The nodes the editor points out: what made the hovered part, the chain of the local view */
  highlightedNodeIds: number[] = $derived.by(() => {
    if (!this.enabled) return [];
    const ids: number[] = [];
    const creator = this.hoveredCreator;
    const chain = creator === undefined ? undefined : this.index?.chains.get(creator);
    const hovered = chain && visibleNode(chain, this.entered);
    if (hovered !== undefined) ids.push(hovered);

    const local = this.local;
    const manager = this.sources.manager();
    if (local && manager && local.entered.join() === this.entered.join()) {
      const edges = manager.edges.map((edge): [number, number] => [edge[0].id, edge[2].id]);
      ids.push(...downstream(edges, local.nodeIds));
    }
    return ids;
  });

  onparthover = (hit: PartHit | undefined) => {
    this.hoveredPart = hit;
  };

  onnodehover = (nodeId: number | undefined) => {
    this.hoveredNode = nodeId;
  };

  /** Selects the node that made the part, with alt the nodes of every path it grows on as well */
  onpartclick = (hit: PartHit | undefined, event: { altKey: boolean }) => {
    const index = this.index;
    if (!this.enabled || !index) return;
    if (!hit) {
      this.picked = undefined;
      return;
    }
    const creator = nodeOfPart(index, this.result, hit);
    const chain = creator === undefined ? undefined : index.chains.get(creator);
    if (creator === undefined || !chain) {
      this.sources.notify('This part does not know which node made it');
      return;
    }

    const nodeId = this.select(chain);
    if (nodeId === undefined) return;
    if (hit.kind === 'groups') {
      // a tissue is as fine as a geometry tells its groups apart
      const creators = hit.creator === undefined ? [] : [hit.creator];
      this.picked = { nodeId, paths: [], creators, instancesOnly: false };
      return;
    }
    if (hit.kind === 'instances' && hit.pathId === NO_PATH && hit.creator !== undefined) {
      // an instance on a point without a path, like a leaf of a rosette, is picked with the
      // others of its node
      this.picked = { nodeId, paths: [], creators: [hit.creator], instancesOnly: false };
      return;
    }
    const instancesOnly = hit.kind === 'instances' && isInstanceNode(index, creator);
    if (!event.altKey) {
      this.picked = { nodeId, paths: [hit.pathId], creators: [], instancesOnly };
      return;
    }

    const links = this.result.links;
    const lineage = lineageOf(links, hit.pathId);
    const state = this.sources.graphState();
    for (const pathId of lineage) {
      const link = links.get(pathId);
      const made = link && index.creators.get(link.creator);
      const visible = made === undefined
        ? undefined
        : visibleNode(index.chains.get(made) ?? [], this.entered);
      if (visible !== undefined && visible !== nodeId) state?.selectedNodes.add(visible);
    }
    this.picked = { nodeId, paths: lineage, creators: [], instancesOnly: false };
  };

  /** Makes the node at the end of `chain` the active one, leaving groups it is not inside of */
  private select(chain: number[]) {
    const manager = this.sources.manager();
    const state = this.sources.graphState();
    if (!manager || !state) return;
    let entered = manager.path.map((entry) => entry.nodeId);
    while (entered.length && visibleNode(chain, entered) === undefined) {
      state.exitGroupNode();
      const left = manager.path.map((entry) => entry.nodeId);
      if (left.length >= entered.length) break;
      entered = left;
    }
    const nodeId = visibleNode(chain, entered);
    const node = nodeId === undefined ? undefined : manager.getNode(nodeId);
    if (!node) return;
    state.clearSelection();
    state.activeNodeId = node.id;
    this.bringIntoView(state, node);
    return node.id;
  }

  /**
   * Moves the editor to the node at the zoom it has. `centerNode` would zoom in until the node
   * fills the view, which loses the place in the graph with every click in the viewer.
   */
  private bringIntoView(state: GraphState, node: { position: [number, number] }) {
    const zoom = state.camera.position[2];
    state.camera.animateTo(
      [node.position[0] + 10, node.position[1] + 10, zoom],
      () => !!state.mouseDown
    );
  }

  toggleLocalView = () => {
    const manager = this.sources.manager();
    if (this.local || !manager) {
      this.requestedLocal = undefined;
      return;
    }
    if (!this.enabled) return;
    const local: LocalView = {
      manager,
      graphId: manager.id,
      entered: [...this.entered],
      nodeIds: [...this.selected],
      picked: this.activePicked
    };
    const parts = local.picked
      ? partsOfPicked(this.result.links, local.picked, true)
      : this.partsOfNodes(local.nodeIds, local.entered);
    if (!parts || isEmpty(parts)) {
      this.sources.notify(
        local.nodeIds.length
          ? 'The selected node made no part that the viewer can tell apart'
          : 'Select a node or click a part to isolate it'
      );
      return;
    }
    this.requestedLocal = local;
  };
}
