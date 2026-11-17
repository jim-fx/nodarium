import { grid } from '#lib/graph-templates/grid.ts';
import { tree } from '#lib/graph-templates/tree.ts';
import { CURRENT_GRAPH_VERSION, validateGraph } from '@nodarium/runtime';
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
    ...import.meta.glob<Graph>('../../../packages/runtime/templates/*.json', {
      eager: true,
      import: 'default'
    }),
    ...import.meta.glob<Graph>('../../benchmark/templates/*.json', {
      eager: true,
      import: 'default'
    })
  };
  // the definitions the nodes are compiled with, the output node keeps its own in inputs.json
  const definitions = new Map(
    Object.values(
      import.meta.glob<NodeDefinition>('../../../nodes/*/*/*/src/{definition,inputs}.json', {
        eager: true,
        import: 'default'
      })
    ).map((d) => [d.id as string, d])
  );

  it.each(Object.entries(graphs))('%s is saved in the current version', (_, graph) => {
    expect(graph.version).toBe(CURRENT_GRAPH_VERSION);
  });

  it.each(Object.entries(graphs))('%s has no issues', (_, graph) => {
    expect(validateGraph(graph, definitions).map((issue) => issue.message)).toEqual([]);
  });
});
