import type { createKeyMap } from '$lib/helpers/createKeyMap';
import { panelState } from '$lib/sidebar/PanelState.svelte';
import FileSaver from 'file-saver';
import type { GraphManager } from './graph-manager.svelte';
import type { GraphState } from './graph-state.svelte';

type Keymap = ReturnType<typeof createKeyMap>;
export function setupKeymaps(keymap: Keymap, graph: GraphManager, graphState: GraphState) {
  keymap.addShortcut({
    key: 'l',
    description: 'Select linked nodes',
    callback: () => {
      const activeNode = graph.getNode(graphState.activeNodeId);
      if (activeNode) {
        const nodes = graph.getLinkedNodes(activeNode);
        graphState.selectedNodes.clear();
        for (const node of nodes) {
          graphState.selectedNodes.add(node.id);
        }
      }
    }
  });

  keymap.addShortcut({
    key: '?',
    description: 'Toggle Help',
    callback: () => {
      panelState.setActivePanel('shortcuts');
    }
  });

  keymap.addShortcut({
    key: 'c',
    ctrl: true,
    description: 'Copy active nodes',
    callback: () => graphState.copyNodes()
  });

  keymap.addShortcut({
    key: 'v',
    ctrl: true,
    description: 'Paste nodes',
    callback: () => graphState.pasteNodes()
  });

  keymap.addShortcut({
    key: 'Escape',
    description: 'Deselect nodes / Exit group',
    callback: () => {
      if (graph.isInsideGroup) {
        const groupId = graph.currentGroupContext;
        if (groupId) {
          graphState.groupCameras.set(
            groupId,
            [...graphState.cameraPosition] as [number, number, number]
          );
        }
        const result = graph.exitGroup();
        if (result !== false) {
          graphState.activeNodeId = result.nodeId;
          graphState.clearSelection();
          graphState.cameraPosition[0] = result.camera[0];
          graphState.cameraPosition[1] = result.camera[1];
          graphState.cameraPosition[2] = result.camera[2];
          return;
        }
      }
      graphState.activeNodeId = -1;
      graphState.clearSelection();
      graphState.edgeEndPosition = null;
      (document.activeElement as HTMLElement)?.blur();
    }
  });

  keymap.addShortcut({
    key: 'A',
    shift: true,
    description: 'Add new Node',
    callback: () => graphState.openNodePalette()
  });

  keymap.addShortcut({
    key: '.',
    description: 'Center camera',
    callback: () => {
      if (!graphState.isBodyFocused()) return;
      graphState.centerNode(graph.getNode(graphState.activeNodeId));
    }
  });

  keymap.addShortcut({
    key: 'a',
    ctrl: true,
    preventDefault: true,
    description: 'Select all nodes',
    callback: () => {
      if (!graphState.isBodyFocused()) return;
      for (const node of graph.nodes.keys()) {
        graphState.selectedNodes.add(node);
      }
    }
  });

  keymap.addShortcut({
    key: 'z',
    ctrl: true,
    description: 'Undo',
    callback: () => {
      if (!graphState.isBodyFocused()) return;
      graph.undo();
      for (const node of graph.nodes.values()) {
        graphState.updateNodePosition(node);
      }
    }
  });

  keymap.addShortcut({
    key: 'y',
    ctrl: true,
    description: 'Redo',
    callback: () => {
      graph.redo();
      for (const node of graph.nodes.values()) {
        graphState.updateNodePosition(node);
      }
    }
  });

  keymap.addShortcut({
    key: 's',
    ctrl: true,
    description: 'Save',
    preventDefault: true,
    callback: () => {
      const state = graph.serialize();
      const blob = new Blob([JSON.stringify(state)], {
        type: 'application/json;charset=utf-8'
      });
      FileSaver.saveAs(blob, 'nodarium-graph.json');
    }
  });

  keymap.addShortcut({
    key: ['Delete', 'Backspace', 'x'],
    description: 'Delete selected nodes',
    callback: (event) => {
      if (!graphState.isBodyFocused()) return;
      graph.startUndoGroup();
      if (graphState.activeNodeId !== -1) {
        const node = graph.getNode(graphState.activeNodeId);
        if (node) {
          graph.removeNode(node, { restoreEdges: event.ctrlKey });
          graphState.activeNodeId = -1;
        }
      }
      if (graphState.selectedNodes) {
        for (const nodeId of graphState.selectedNodes) {
          const node = graph.getNode(nodeId);
          if (node) {
            graph.removeNode(node, { restoreEdges: event.ctrlKey });
          }
        }
        graphState.clearSelection();
      }
      graph.saveUndoGroup();
    }
  });

  keymap.addShortcut({
    key: 'f',
    description: 'Smart Connect Nodes',
    callback: () => {
      const nodes = [...graphState.selectedNodes.values()]
        .map((g) => graph.getNode(g))
        .filter((n) => !!n);
      const edge = graph.smartConnect(nodes[0], nodes[1]);
      if (!edge) graph.smartConnect(nodes[1], nodes[0]);
    }
  });

  keymap.addShortcut({
    key: 'g',
    ctrl: true,
    preventDefault: true,
    description: 'Group selected nodes',
    callback: () => {
      if (!graphState.isBodyFocused()) return;
      const nodeIds = Array.from(
        new Set([
          ...(graphState.selectedNodes.size > 0 ? graphState.selectedNodes.values() : []),
          ...(graphState.activeNodeId !== -1 ? [graphState.activeNodeId] : [])
        ])
      );
      if (nodeIds.length === 0) return;
      const groupNode = graph.createGroup(nodeIds);
      if (groupNode) {
        graphState.selectedNodes.clear();
        graphState.activeNodeId = groupNode.id;
      }
    }
  });

  keymap.addShortcut({
    key: 'g',
    alt: true,
    shift: true,
    preventDefault: true,
    description: 'Ungroup selected node',
    callback: () => {
      if (!graphState.isBodyFocused()) return;
      const nodeId = graphState.activeNodeId !== -1
        ? graphState.activeNodeId
        : graphState.selectedNodes.size === 1
        ? [...graphState.selectedNodes.values()][0]
        : -1;
      if (nodeId === -1) return;
      graph.ungroup(nodeId);
      graphState.activeNodeId = -1;
      graphState.clearSelection();
    }
  });

  keymap.addShortcut({
    key: 'Tab',
    preventDefault: true,
    description: 'Enter focused group node',
    callback: () => {
      if (!graphState.isBodyFocused()) return;
      const entered = graph.enterGroup(
        graphState.activeNodeId,
        [...graphState.cameraPosition] as [number, number, number]
      );
      if (entered) {
        graphState.activeNodeId = -1;
        graphState.clearSelection();
        // Restore group-specific camera if we've been here before, else snap to center
        const groupId = graph.currentGroupContext;
        const saved = groupId ? graphState.groupCameras.get(groupId) : undefined;
        if (saved) {
          graphState.cameraPosition[0] = saved[0];
          graphState.cameraPosition[1] = saved[1];
          graphState.cameraPosition[2] = saved[2];
        } else {
          const nodes = [...graph.nodes.values()];
          if (nodes.length) {
            const avgX = nodes.reduce((s, n) => s + n.position[0], 0) / nodes.length;
            const avgY = nodes.reduce((s, n) => s + n.position[1], 0) / nodes.length;
            graphState.cameraPosition[0] = avgX;
            graphState.cameraPosition[1] = avgY;
            graphState.cameraPosition[2] = 10;
          }
        }
      }
    }
  });
}
