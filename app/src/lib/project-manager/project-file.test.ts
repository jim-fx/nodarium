import { CURRENT_GRAPH_VERSION } from '#lib/graph-migrations/index.ts';
import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { createProjectFile, parseProjectFile } from './project-file';

const graph: Graph = {
  id: 3,
  meta: { title: 'Fern' },
  settings: {},
  nodes: [{ id: 0, type: 'max/plantarium/output', position: [0, 0] }],
  edges: [],
  groups: []
};

describe('parseProjectFile', () => {
  it('parses a project file', () => {
    const preview = 'data:image/webp;base64,AAAA';
    const file = parseProjectFile({ format: 'nodarium-project', version: 1, graph, preview });
    expect(file.graph.meta?.title).toBe('Fern');
    expect(file.preview).toBe(preview);
  });

  it('accepts a bare graph', () => {
    const file = parseProjectFile(graph);
    expect(file.format).toBe('nodarium-project');
    expect(file.graph.nodes).toHaveLength(1);
    expect(file.preview).toBeUndefined();
  });

  it('fills in a missing id and groups', () => {
    const partial: Partial<Graph> = structuredClone(graph);
    delete partial.id;
    delete partial.groups;
    const file = parseProjectFile({ format: 'nodarium-project', version: 1, graph: partial });
    expect(file.graph.id).toBe(0);
    expect(file.graph.groups).toEqual([]);
  });

  it('migrates the graph', () => {
    const old: Partial<Graph> = structuredClone(graph);
    delete old.groups;
    const file = parseProjectFile(old);
    expect(file.graph.version).toBe(CURRENT_GRAPH_VERSION);
  });

  it('rejects graphs from a newer version of the app', () => {
    expect(() => parseProjectFile({ ...graph, version: CURRENT_GRAPH_VERSION + 1 })).toThrow(
      /newer version/
    );
  });

  it('rejects previews that are not images', () => {
    expect(() =>
      parseProjectFile({
        format: 'nodarium-project',
        version: 1,
        graph,
        preview: 'javascript:alert(1)'
      })
    ).toThrow(/preview/);
  });

  it('reports errors of the project file schema', () => {
    expect(() => parseProjectFile({ format: 'nodarium-project', version: 2, graph })).toThrow(
      /version/
    );
  });
});

describe('createProjectFile', () => {
  it('creates a project file without a preview', async () => {
    const file = await createProjectFile(graph);
    expect(file).toEqual({ format: 'nodarium-project', version: 1, graph, preview: undefined });
  });
});
