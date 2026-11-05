import { type NodeDefinition, NodeDefinitionSchema } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { getParameterHeight } from './nodeHelpers';

const definition = {
  id: 'max/myzel/to_mesh',
  inputs: {
    field: { type: 'field' },
    base: { type: 'mesh', accepts: ['field'] },
    size: { type: 'float', value: 1 }
  },
  outputs: ['mesh', 'field']
};

describe('field and mesh sockets', () => {
  it('validate as node definition', () => {
    expect(NodeDefinitionSchema.safeParse(definition).success).toBe(true);
  });

  it('show as a plain socket without an input field', () => {
    const node = { ...definition, execute: () => new Int32Array() } as NodeDefinition;
    expect(getParameterHeight(node, 'field')).toBe(50);
    expect(getParameterHeight(node, 'base')).toBe(50);
    expect(getParameterHeight(node, 'size')).toBe(100);
  });
});
