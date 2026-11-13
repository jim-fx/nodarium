import type { Graph } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { createProjectFile } from './project-file';

const graph: Graph = {
  id: 3,
  meta: { title: 'Fern' },
  settings: {},
  nodes: [{ id: 0, type: 'max/plantarium/output', position: [0, 0] }],
  edges: [],
  groups: []
};

describe('createProjectFile', () => {
  it('creates a project file without a preview', async () => {
    const file = await createProjectFile(graph);
    expect(file).toEqual({ format: 'nodarium-project', version: 1, graph, preview: undefined });
  });
});
