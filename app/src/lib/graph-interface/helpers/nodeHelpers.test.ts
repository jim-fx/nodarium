import { type NodeDefinition, NodeDefinitionSchema } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { getParameterHeight } from './nodeHelpers';

const definition = {
  id: 'max/myzel/to_mesh',
  inputs: {
    base: { type: 'mesh', accepts: ['pattern'] },
    hyphae: { type: 'hyphae' },
    pattern: { type: 'pattern' },
    size: { type: 'float', value: 1 }
  },
  outputs: ['mesh', 'hyphae', 'pattern']
};

describe('mesh, hyphae and pattern sockets', () => {
  it('validate as node definition', () => {
    expect(NodeDefinitionSchema.safeParse(definition).success).toBe(true);
  });

  it('show as a plain socket without an input field', () => {
    const node = { ...definition, execute: () => new Int32Array() } as NodeDefinition;
    expect(getParameterHeight(node, 'base')).toBe(50);
    expect(getParameterHeight(node, 'hyphae')).toBe(50);
    expect(getParameterHeight(node, 'pattern')).toBe(50);
    expect(getParameterHeight(node, 'size')).toBe(100);
  });
});
