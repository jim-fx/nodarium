import type { createKeyMap } from '$lib/helpers/createKeyMap';
import { panelState } from '$lib/sidebar/PanelState.svelte';
import { toast } from '@nodarium/ui';
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
    description: 'Deselect nodes',
    callback: () => {
      if (graph.isInsideGroup) {
        graphState.exitGroupNode();
        return;
      }
      graphState.activeNodeId = -1;
      graphState.clearSelection();
      graphState.edgeEndPosition = null;
      (document.activeElement as HTMLElement)?.blur();
    }
  });

  keymap.addShortcut({
    key: 'g',
    ctrl: true,
    preventDefault: true,
    description: 'Group selected nodes',
    callback: () => graphState.groupSelectedNodes()
  });

  keymap.addShortcut({
    key: 'g',
    alt: true,
    preventDefault: true,
    description: 'Ungroup selected nodes',
    callback: () => graphState.unGroupSelectedNodes()
  });

  keymap.addShortcut({
    key: 'Tab',
    preventDefault: true,
    description: 'Enter selected node group',
    callback: () => graphState.enterGroupNode()
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
      toast('Graph downloaded', 'success', 1500);
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
}
