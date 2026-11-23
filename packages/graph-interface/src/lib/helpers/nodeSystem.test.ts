import { GROUP_INPUT, GROUP_INSTANCE } from '@nodarium/runtime';
import { describe, expect, it } from 'vitest';
import { nodeSystem, systemColor } from './nodeSystem';

const colors = { 'max/plantarium': '#6f8a52', 'max/myzel': '#9a7650' };

describe('nodeSystem', () => {
  it('names the system of a node type', () => {
    expect(nodeSystem('max/plantarium/stem')).toEqual({
      collection: 'max/plantarium',
      name: 'plantarium'
    });
    expect(nodeSystem('max/core/float')?.name).toBe('core');
  });

  it('has none for groups', () => {
    expect(nodeSystem(GROUP_INSTANCE)).toBeUndefined();
    expect(nodeSystem(GROUP_INPUT)).toBeUndefined();
  });
});

describe('systemColor', () => {
  it('gives the colour of the collection', () => {
    expect(systemColor('max/plantarium/stem', colors)).toBe('#6f8a52');
    expect(systemColor('max/myzel/strands', colors)).toBe('#9a7650');
  });

  it('gives none for core, groups and unknown collections', () => {
    expect(systemColor('max/core/float', colors)).toBeUndefined();
    expect(systemColor(GROUP_INSTANCE, colors)).toBeUndefined();
    expect(systemColor('alice/trees/oak', colors)).toBeUndefined();
    expect(systemColor('max/plantarium/stem', undefined)).toBeUndefined();
  });
});
