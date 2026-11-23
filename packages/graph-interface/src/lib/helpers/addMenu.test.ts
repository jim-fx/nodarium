import type { NodeDefinition, NodeId } from '@nodarium/runtime';
import { describe, expect, it } from 'vitest';
import { mockFloatInputNode } from '../test-utils';
import { groupNodes, nodeLabel, searchNodes, sortByLabel } from './addMenu';

function node(id: NodeId, title?: string): NodeDefinition {
  return { ...mockFloatInputNode, id, meta: title ? { title } : undefined };
}

const nodes = [
  node('max/plantarium/stem'),
  node('max/plantarium/random_break'),
  node('max/plantarium/output'),
  node('max/myzel/output'),
  node('max/myzel/cap'),
  node('max/core/random'),
  node('max/core/random_int'),
  node('__internal/node/debug', 'Debug')
];

const ids = (list: NodeDefinition[]) => list.map((n) => n.id);

describe('nodeLabel', () => {
  it('is the title, or the last part of the id', () => {
    expect(nodeLabel(node('max/core/random'))).toBe('random');
    expect(nodeLabel(node('__internal/node/debug', 'Debug'))).toBe('Debug');
  });
});

describe('searchNodes', () => {
  it('ignores case and matches the label', () => {
    expect(ids(searchNodes(nodes, 'Random'))).toEqual([
      'max/plantarium/random_break',
      'max/core/random',
      'max/core/random_int'
    ]);
    expect(ids(searchNodes(nodes, 'DEBUG'))).toEqual(['__internal/node/debug']);
  });

  it('matches the id, a label that starts with the query comes first', () => {
    expect(ids(searchNodes(nodes, 'myzel'))).toEqual(['max/myzel/output', 'max/myzel/cap']);
    expect(ids(searchNodes(nodes, 'break'))).toEqual(['max/plantarium/random_break']);
    expect(ids(searchNodes([node('max/x/fill_gaps'), node('max/x/gap')], 'gap'))).toEqual([
      'max/x/gap',
      'max/x/fill_gaps'
    ]);
  });

  it('gives every node for an empty query', () => {
    expect(searchNodes(nodes, '  ')).toBe(nodes);
  });

  it('gives nothing when nothing matches', () => {
    expect(searchNodes(nodes, 'zzz')).toEqual([]);
  });
});

describe('groupNodes', () => {
  it('has a section per collection, core first with the internal nodes', () => {
    const sections = groupNodes(sortByLabel(nodes));
    expect(sections.map((s) => s.title)).toEqual(['core', 'myzel', 'plantarium']);
    expect(ids(sections[0].nodes)).toEqual([
      '__internal/node/debug',
      'max/core/random',
      'max/core/random_int'
    ]);
    expect(ids(sections[2].nodes)).toEqual([
      'max/plantarium/output',
      'max/plantarium/random_break',
      'max/plantarium/stem'
    ]);
  });

  it('puts the two nodes with the same label under different headings', () => {
    const sections = groupNodes(searchNodes(nodes, 'output'));
    expect(sections.map((s) => [s.title, ids(s.nodes)])).toEqual([
      ['myzel', ['max/myzel/output']],
      ['plantarium', ['max/plantarium/output']]
    ]);
  });

  it('names a section by its whole id when two users have a collection of that name', () => {
    const sections = groupNodes([node('max/plants/a'), node('ana/plants/b')]);
    expect(sections.map((s) => s.title)).toEqual(['ana/plants', 'max/plants']);
  });

  it('keeps the order of the search within a section', () => {
    const sections = groupNodes(searchNodes(sortByLabel(nodes), 'random'));
    expect(ids(sections[0].nodes)).toEqual(['max/core/random', 'max/core/random_int']);
  });
});
