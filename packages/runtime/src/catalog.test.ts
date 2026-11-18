import type { NodeDefinition } from '@nodarium/types';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import {
  findNodes,
  firstSentence,
  formatNode,
  formatNodeList,
  nodeSignature,
  nodesOfType,
  nodeSummary
} from './catalog';
import { listNodes } from './list-nodes';

const nodesDir = fileURLToPath(new URL('../../../app/static/nodes', import.meta.url));

let nodes: NodeDefinition[];
beforeAll(async () => {
  nodes = await listNodes(nodesDir);
});

describe('the nodes', () => {
  it('all have a description', () => {
    const missing = nodes.filter((node) => !node.meta?.description).map((node) => node.id);
    expect(missing).toEqual([]);
  });

  // the hidden ones are set by templates, a seed and a creator are never set by hand
  it('describe every input that is shown', () => {
    const missing = nodes.flatMap((node) =>
      Object.entries(node.inputs ?? {})
        .filter(([, input]) => !input.hidden && input.type !== 'seed' && input.type !== 'creator')
        .filter(([, input]) => !input.description)
        .map(([key]) => `${node.id} ${key}`)
    );
    expect(missing).toEqual([]);
  });
});

describe('the catalog', () => {
  const noise = () => findNodes(nodes, 'noise')[0];

  it('lists every node of the package, sorted by id', () => {
    const ids = nodes.map((node) => node.id);
    expect(ids).toContain('max/plantarium/stem');
    expect(ids).toContain('max/myzel/cap');
    expect(ids).toEqual([...ids].sort((a, b) => a.localeCompare(b)));
    expect(ids.some((id) => id.startsWith('__internal'))).toBe(false);
  });

  it('writes a node as its data inputs and outputs', () => {
    expect(nodeSignature(noise())).toBe('max/plantarium/noise (path) -> path');
    expect(nodeSignature(findNodes(nodes, 'max/plantarium/output')[0])).toBe(
      'max/plantarium/output (path)'
    );
  });

  it('sums a node up as data', () => {
    expect(nodeSummary(noise())).toEqual({
      id: 'max/plantarium/noise',
      inputs: ['path'],
      outputs: ['path'],
      description: 'Bends paths with smooth noise, more toward the tip.'
    });
  });

  it('gives a block per node with an input per line and its description', () => {
    const block = formatNode(noise()).split('\n');
    expect(block[0]).toBe('max/plantarium/noise (path) -> path');
    expect(block).toContain('  scale: float = 1 [0.1..10]');
    expect(block).toContain('  fix_bottom: float = 1 [0..1] (hidden)');
    expect(block).toContain('  seed: seed');
    expect(formatNode(findNodes(nodes, 'rotate')[0])).toContain(
      '  axis: select = x (0 x, 1 y, 2 z)'
    );
    expect(formatNode(findNodes(nodes, 'max/plantarium/stem')[0])).toContain(
      '  length: float = 5 m'
    );
    expect(formatNode(noise(), { descriptions: false })).not.toContain(
      noise().inputs!.scale.description!
    );
  });

  it('lists the nodes with the first sentence of their description', () => {
    const list = formatNodeList([noise()]);
    expect(list).toBe(
      `max/plantarium/noise (path) -> path\n  ${firstSentence(noise().meta!.description!)}`
    );
    expect(firstSentence('Bends a path. The same seed gives the same bend.')).toBe(
      'Bends a path.'
    );
    expect(firstSentence('Cuts at 0.5 m. Then more')).toBe('Cuts at 0.5 m.');
  });

  it('finds a node by its name in every collection that has it', () => {
    expect(findNodes(nodes, 'output').map((node) => node.id)).toEqual([
      'max/myzel/output',
      'max/plantarium/output'
    ]);
    expect(findNodes(nodes, 'nope')).toEqual([]);
  });

  it('finds the nodes that take or give a type', () => {
    const meshes = nodesOfType(nodes, 'geometry').map((node) => node.id);
    expect(meshes).toContain('max/myzel/hyphae_to_mesh');
    expect(meshes).toContain('max/myzel/output');
    expect(meshes).not.toContain('max/plantarium/stem');
  });
});
