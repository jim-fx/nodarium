import type { Graph, NodeDefinition } from '@nodarium/types';
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { encodeText, getValue } from './inputs';
import { validateGraph } from './validate-graph';

const lsystem: NodeDefinition = JSON.parse(
  readFileSync(
    new URL('../../../nodes/max/plantarium/lsystem/src/definition.json', import.meta.url),
    'utf8'
  )
);

describe('text inputs', () => {
  it('cross into the wasm as one word per code point', () => {
    expect(encodeText('F→[+F]')).toEqual([0, 7, 70, 8594, 91, 43, 70, 93, 1, 1]);
    // a character outside of the basic plane is one code point, not two halves
    expect(encodeText('🌿')).toEqual([0, 2, 0x1f33f, 1, 1]);
    expect(encodeText('')).toEqual([0, 1, 1, 1]);
  });

  it('take the value set on the node or the default of the input', () => {
    const rules = lsystem.inputs!.rules;
    expect(getValue(rules, 'A = FA')).toEqual(encodeText('A = FA'));
    expect(getValue(rules)).toEqual(encodeText(rules.value as string));
  });

  it('are written into a graph as a string', () => {
    const graph = (axiom: unknown): Graph => ({
      id: 0,
      nodes: [
        { id: 1, type: 'max/plantarium/lsystem', position: [0, 0], props: { axiom } as never }
      ],
      edges: [],
      groups: []
    });
    const definitions = new Map([[lsystem.id as string, lsystem]]);
    const issues = (axiom: unknown) =>
      validateGraph(graph(axiom), definitions).filter(i => i.code === 'wrong-value-type');
    expect(issues('FFA')).toEqual([]);
    expect(issues([1, 2])[0].message).toBe(
      'lsystem#1 "axiom" is [1,2], it takes a text in quotes'
    );
  });
});
