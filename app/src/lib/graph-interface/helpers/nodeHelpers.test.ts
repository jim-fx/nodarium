import { GROUP_INPUT, type NodeDefinition, NodeDefinitionSchema } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { exposeInputs } from './group-types';
import { getNodeRows, getParameterHeight, headerSocketSide, inputSocketSide } from './nodeHelpers';

const definition = {
  id: 'max/myzel/to_mesh',
  inputs: {
    base: { type: 'mesh', accepts: ['pattern'] },
    pattern: { type: 'pattern' },
    size: { type: 'float', value: 1 }
  },
  outputs: ['mesh', 'pattern']
};

describe('mesh and pattern sockets', () => {
  it('validate as node definition', () => {
    expect(NodeDefinitionSchema.safeParse(definition).success).toBe(true);
  });

  it('show as a plain socket without an input field', () => {
    const node = { ...definition, execute: () => new Int32Array() } as NodeDefinition;
    expect(getParameterHeight(node, 'base')).toBe(50);
    expect(getParameterHeight(node, 'pattern')).toBe(50);
    expect(getParameterHeight(node, 'size')).toBe(100);
  });
});

describe('text inputs', () => {
  const node = {
    id: 'max/plantarium/lsystem',
    inputs: {
      axiom: { type: 'text', internal: true, value: 'A' },
      rules: { type: 'text', internal: true, multiline: true, value: 'A = F[+A]' }
    },
    outputs: ['path']
  };

  it('validate as node definition', () => {
    expect(NodeDefinitionSchema.safeParse(node).success).toBe(true);
  });

  it('get a field of one line, or a taller one for several lines', () => {
    const definition = { ...node, execute: () => new Int32Array() } as NodeDefinition;
    expect(getParameterHeight(definition, 'axiom')).toBe(100);
    expect(getParameterHeight(definition, 'rules')).toBe(180);
  });
});

describe('getNodeRows', () => {
  const stem = {
    id: 'max/plantarium/stem',
    inputs: {
      origin: { type: 'vec3', value: [0, 0, 0] },
      shape: { type: 'shape', internal: true },
      length: { type: 'float', value: 1, hidden: true },
      amount: { type: 'integer', value: 1 },
      seed: { type: 'seed' },
      resolution: { type: 'integer', setting: 'resolution.curve' }
    },
    outputs: ['path', 'float'],
    execute: () => new Int32Array()
  } as unknown as NodeDefinition;

  it('gives a row to every output after the first, then to every visible input', () => {
    expect(getNodeRows(stem.id, stem)).toEqual([
      { height: 5, socket: 'right', socketIndex: 1 },
      { height: 10, socket: 'left', socketIndex: 'origin' },
      { height: 20, socket: undefined, socketIndex: 'shape' },
      { height: 10, socket: 'left', socketIndex: 'amount' }
    ]);
  });

  it('gives an exposed hidden input a row with a socket', () => {
    const rows = getNodeRows(stem.id, exposeInputs(stem, ['length']));
    expect(rows.map((row) => row.socketIndex)).toEqual([1, 'origin', 'shape', 'length', 'amount']);
    expect(rows[3].socket).toBe('left');
  });

  it('puts the sockets of the Group Input node on the right, indexed as outputs', () => {
    const groupInput = {
      id: GROUP_INPUT,
      inputs: { a: { type: 'float' }, b: { type: 'path' } },
      outputs: [],
      execute: () => new Int32Array()
    } as unknown as NodeDefinition;
    expect(getNodeRows(GROUP_INPUT, groupInput)).toEqual([
      { height: 5, socket: 'right', socketIndex: 0 },
      { height: 5, socket: 'right', socketIndex: 1 }
    ]);
    expect(headerSocketSide(GROUP_INPUT, { ...groupInput, outputs: ['float'] })).toBeUndefined();
  });

  it('shows a header socket only for a node with outputs', () => {
    expect(headerSocketSide(stem.id, stem)).toBe('right');
    expect(headerSocketSide('max/core/output', { ...stem, outputs: [] })).toBeUndefined();
  });

  it('shows no socket on an internal input', () => {
    expect(inputSocketSide(stem.id, { internal: true })).toBeUndefined();
    expect(inputSocketSide(stem.id, {})).toBe('left');
  });
});
