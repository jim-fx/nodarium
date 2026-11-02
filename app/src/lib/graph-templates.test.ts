import { CURRENT_GRAPH_VERSION } from '#lib/graph-migrations/index.ts';
import { grid } from '#lib/graph-templates/grid.ts';
import { tree } from '#lib/graph-templates/tree.ts';
import type { Graph, NodeDefinition } from '@nodarium/types';
import { describe, expect, it } from 'vitest';

describe('graph-templates', () => {
  describe('grid', () => {
    it('should create a grid graph with nodes and edges', () => {
      const result = grid(2, 3);
      expect(result.nodes.length).toBeGreaterThan(0);
      expect(result.edges.length).toBeGreaterThan(0);
    });

    it('should have output node at the end', () => {
      const result = grid(1, 1);
      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
    });

    it('should create nodes based on grid dimensions', () => {
      const result = grid(2, 2);
      const mathNodes = result.nodes.filter(n => n.type === 'max/plantarium/math');
      expect(mathNodes.length).toBeGreaterThan(0);
      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
    });

    it('should have output node at the end', () => {
      const result = grid(1, 1);
      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
    });

    it('should create nodes based on grid dimensions', () => {
      const result = grid(2, 2);
      const mathNodes = result.nodes.filter(n => n.type === 'max/plantarium/math');
      expect(mathNodes.length).toBeGreaterThan(0);
      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
    });

    it('should have valid node positions', () => {
      const result = grid(3, 2);

      result.nodes.forEach(node => {
        expect(node.position).toHaveLength(2);
        expect(typeof node.position[0]).toBe('number');
        expect(typeof node.position[1]).toBe('number');
      });
    });

    it('should generate valid graph structure', () => {
      const result = grid(2, 2);

      result.nodes.forEach(node => {
        expect(typeof node.id).toBe('number');
        expect(node.type).toBeTruthy();
      });

      result.edges.forEach(edge => {
        expect(edge).toHaveLength(4);
      });
    });
  });

  describe('tree', () => {
    it('should create a tree graph with specified depth', () => {
      const result = tree(0);

      expect(result.nodes.length).toBeGreaterThan(0);
      expect(result.edges.length).toBeGreaterThan(0);
    });

    it('should have root output node', () => {
      const result = tree(2);

      const outputNode = result.nodes.find(n => n.type === 'max/plantarium/output');
      expect(outputNode).toBeDefined();
      expect(outputNode?.id).toBe(0);
    });

    it('should increase node count with depth', () => {
      const tree0 = tree(0);
      const tree1 = tree(1);
      const tree2 = tree(2);

      expect(tree0.nodes.length).toBeLessThan(tree1.nodes.length);
      expect(tree1.nodes.length).toBeLessThan(tree2.nodes.length);
    });

    it('should create binary tree structure', () => {
      const result = tree(2);

      const mathNodes = result.nodes.filter(n => n.type === 'max/plantarium/math');
      expect(mathNodes.length).toBeGreaterThan(0);

      const edgeCount = result.edges.length;
      expect(edgeCount).toBe(result.nodes.length - 1);
    });

    it('should have valid node positions', () => {
      const result = tree(3);

      result.nodes.forEach(node => {
        expect(node.position).toHaveLength(2);
        expect(typeof node.position[0]).toBe('number');
        expect(typeof node.position[1]).toBe('number');
      });
    });
  });
});

describe('bundled graphs', () => {
  const graphs = {
    ...import.meta.glob<Graph>('./graph-templates/*.json', { eager: true, import: 'default' }),
    ...import.meta.glob<Graph>('../../benchmark/templates/*.json', {
      eager: true,
      import: 'default'
    })
  };
  const definitions = Object.values(
    import.meta.glob<NodeDefinition>('../../../nodes/*/*/*/src/definition.json', {
      eager: true,
      import: 'default'
    })
  );
  const inputsOf = new Map(definitions.map((d) => [d.id as string, Object.keys(d.inputs ?? {})]));

  it.each(Object.entries(graphs))('%s is saved in the current version', (_, graph) => {
    expect(graph.version).toBe(CURRENT_GRAPH_VERSION);
  });

  it.each(Object.entries(graphs))('%s only uses inputs its nodes have', (_, graph) => {
    const unknown: string[] = [];
    for (const scope of [graph, ...graph.groups]) {
      const types = new Map(scope.nodes.map((n) => [n.id, n.type as string]));
      const check = (id: number, input: string) => {
        const inputs = inputsOf.get(types.get(id)!);
        if (inputs && !inputs.includes(input)) unknown.push(`${types.get(id)} ${input}`);
      };
      for (const node of scope.nodes) {
        for (const input of [...Object.keys(node.props ?? {}), ...(node.exposed ?? [])]) {
          check(node.id, input);
        }
      }
      for (const edge of scope.edges) check(edge[2], edge[3]);
    }
    expect([...new Set(unknown)]).toEqual([]);
  });
});
