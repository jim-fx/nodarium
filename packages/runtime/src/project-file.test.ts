import type { Graph } from '@nodarium/types';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';
import { CURRENT_GRAPH_VERSION } from './graph-migrations';
import { graphFileJsonSchema, parseProjectFile } from './project-file';

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

  it('accepts node types of a collection with sub-namespaces, not of a collection alone', () => {
    const nested = { ...graph, nodes: [{ id: 0, type: 'max/vj/fx/bloom', position: [0, 0] }] };
    expect(parseProjectFile(nested).graph.nodes[0].type).toBe('max/vj/fx/bloom');
    const short = { ...graph, nodes: [{ id: 0, type: 'max/vj', position: [0, 0] }] };
    expect(() => parseProjectFile(short)).toThrow();
  });

  it('migrates the graph', () => {
    const old: Partial<Graph> = structuredClone(graph);
    delete old.groups;
    const file = parseProjectFile(old);
    expect(file.graph.version).toBe(CURRENT_GRAPH_VERSION);
  });

  it('rejects graphs from a newer version of the app', () => {
    expect(() => parseProjectFile({ ...graph, version: CURRENT_GRAPH_VERSION + 1 })).toThrow(
      /Update @nodarium\/runtime/
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

  it('keeps the frames of the graph and of its groups', () => {
    const frame = { id: 1, label: 'Leaves', color: 'green' as const, nodes: [0] };
    const group = { id: 5, nodes: [], edges: [], frames: [{ id: 1, label: 'Inner', nodes: [] }] };
    const file = parseProjectFile({ ...graph, frames: [frame], groups: [group] });
    expect(file.graph.frames).toEqual([frame]);
    expect(file.graph.groups[0].frames).toEqual(group.frames);
  });

  it('reads a frame colour it does not know as none', () => {
    const file = parseProjectFile({
      ...graph,
      frames: [{ id: 1, label: 'Leaves', color: 'teal', nodes: [0] }]
    });
    expect(file.graph.frames).toEqual([{ id: 1, label: 'Leaves', nodes: [0] }]);
  });

  it('reports errors of the project file schema', () => {
    expect(() => parseProjectFile({ format: 'nodarium-project', version: 2, graph })).toThrow(
      /version/
    );
  });
});

describe('graphFileJsonSchema', () => {
  // zod reads JSON Schema back, a validator of another library takes the same schema
  const schema = z.fromJSONSchema(graphFileJsonSchema() as Parameters<typeof z.fromJSONSchema>[0]);
  const dir = join(import.meta.dirname, '../templates');

  it('accepts every template, a project file and a graph written by hand', () => {
    for (const file of readdirSync(dir).filter((f) => f.endsWith('.json'))) {
      const template = JSON.parse(readFileSync(join(dir, file), 'utf8'));
      expect(schema.safeParse(template).success, file).toBe(true);
    }
    const preview = 'data:image/webp;base64,AAAA';
    expect(schema.safeParse({ format: 'nodarium-project', version: 1, graph, preview }).success)
      .toBe(true);
    expect(schema.safeParse({ nodes: [{ id: 1, type: 'max/plantarium/stem' }], edges: [] }).success)
      .toBe(true);
    const frames = [{ id: 1, label: 'Stem', color: 'brown', nodes: [1] }];
    expect(
      schema.safeParse({ nodes: [{ id: 1, type: 'max/plantarium/stem' }], edges: [], frames })
        .success
    ).toBe(true);
  });

  it('rejects what parseProjectFile rejects', () => {
    expect(schema.safeParse({ nodes: [{ id: 1, type: 'stem' }], edges: [] }).success).toBe(false);
    expect(schema.safeParse({ format: 'nodarium-project', version: 2, graph }).success).toBe(false);
    expect(schema.safeParse({ edges: [] }).success).toBe(false);
  });
});
