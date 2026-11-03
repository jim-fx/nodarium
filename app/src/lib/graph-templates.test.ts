import { CURRENT_GRAPH_VERSION } from '#lib/graph-migrations/index.ts';
import { grid } from '#lib/graph-templates/grid.ts';
import { tree } from '#lib/graph-templates/tree.ts';
import type { Graph, NodeDefinition } from '@nodarium/types';
import { describe, expect, it } from 'vitest';

describe('graph-templates', () => {
  it('grid has an output node', () => {
    const result = grid(2, 2);
    expect(result.nodes.some(n => n.type === 'max/plantarium/output')).toBe(true);
  });

  it('tree has the output node as its root', () => {
    const result = tree(2);
    expect(result.nodes.find(n => n.type === 'max/plantarium/output')?.id).toBe(0);
  });

  it('tree grows with its depth', () => {
    expect(tree(0).nodes.length).toBeLessThan(tree(1).nodes.length);
    expect(tree(1).nodes.length).toBeLessThan(tree(2).nodes.length);
  });

  it('tree connects every node but the root once', () => {
    const result = tree(2);
    expect(result.edges.length).toBe(result.nodes.length - 1);
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
