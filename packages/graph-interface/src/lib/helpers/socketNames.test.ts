import {
  GROUP_INPUT,
  GROUP_OUTPUT,
  type GroupDefinition,
  type NodeDefinition
} from '@nodarium/runtime';
import { describe, expect, it } from 'vitest';
import { groupInputType, groupOutputType } from './group-types';
import { connectionDescription, nodeLabel, socketName } from './socketNames';

const stem = { id: 10, type: 'max/plantarium/stem' } as const;
const stemType = {
  id: 'max/plantarium/stem',
  inputs: { amount: { type: 'integer' }, thickness: { type: 'float', label: 'Thickness' } },
  outputs: ['path'],
  execute: () => new Int32Array()
} as unknown as NodeDefinition;

const noise = { id: 15, type: 'max/plantarium/noise' } as const;
const noiseType = {
  id: 'max/plantarium/noise',
  inputs: { plant: { type: 'path' } },
  outputs: [{ type: 'path', label: 'noisy' }, 'float'],
  meta: { title: 'Noise' },
  execute: () => new Int32Array()
} as unknown as NodeDefinition;

const group = {
  id: 3,
  name: 'Leaf',
  inputs: { a: { type: 'path', label: 'plant' }, b: { type: 'float', label: 'size' } },
  outputs: [{ type: 'path', label: 'leaf' }],
  nodes: [],
  edges: []
} as unknown as GroupDefinition;

describe('nodeLabel', () => {
  it('is the last part of the type without a title', () => {
    expect(nodeLabel(stem, stemType)).toBe('stem, node 10');
  });

  it('is the title of the node', () => {
    expect(nodeLabel(noise, noiseType)).toBe('Noise, node 15');
  });

  it('is the last part of the type while the type is not loaded', () => {
    expect(nodeLabel(stem, undefined)).toBe('stem, node 10');
  });
});

describe('socketName', () => {
  it('names an output by its type', () => {
    expect(socketName(stem, stemType, 0)).toBe('output path of stem');
  });

  it('names an output by its label', () => {
    expect(socketName(noise, noiseType, 0)).toBe('output noisy of Noise');
    expect(socketName(noise, noiseType, 1)).toBe('output float of Noise');
  });

  it('names an input by its label, or by its key without one', () => {
    expect(socketName(stem, stemType, 'thickness')).toBe('input Thickness of stem');
    expect(socketName(stem, stemType, 'amount')).toBe('input amount of stem');
  });

  it('names the outputs of the Group Input node after the inputs of the group', () => {
    const node = { id: 1, type: GROUP_INPUT } as const;
    const type = groupInputType(group);
    expect(socketName(node, type, 0)).toBe('output plant of Group Input');
    expect(socketName(node, type, 1)).toBe('output size of Group Input');
  });

  it('names the inputs of the Group Output node after the outputs of the group', () => {
    const node = { id: 2, type: GROUP_OUTPUT } as const;
    expect(socketName(node, groupOutputType(group), 'out_0')).toBe('input leaf of Group Output');
  });
});

describe('connectionDescription', () => {
  it('names the output that feeds an input and its node', () => {
    expect(connectionDescription(stem, stemType, 0)).toBe(
      'connected to output path of stem (#10)'
    );
  });
});
