import { GROUP_INSTANCE, groupInputKey, groupInputValue, groupOutputIndex } from '@nodarium/types';
import { describe, expect, it } from 'vitest';

// input_1 was removed: the names keep their gap, the sockets move up
const group = {
  inputs: {
    input_0: { type: 'float', value: 0.5 },
    input_2: { type: 'float' },
    input_3: { type: 'vec3', value: [0, 1, 0] }
  }
};

const instance = (props?: Record<string, unknown>) => ({ type: GROUP_INSTANCE, props });

describe('the boundary of a group', () => {
  describe('groupInputKey', () => {
    it('is the N-th input of the group for output socket N of the Group Input node', () => {
      expect(groupInputKey(group, 0)).toBe('input_0');
      expect(groupInputKey(group, 1)).toBe('input_2');
      expect(groupInputKey(group, 2)).toBe('input_3');
    });

    it('is undefined for a socket the group has no input for', () => {
      expect(groupInputKey(group, 3)).toBeUndefined();
      expect(groupInputKey(group, -1)).toBeUndefined();
      expect(groupInputKey({}, 0)).toBeUndefined();
    });
  });

  describe('groupOutputIndex', () => {
    it('is N for the input out_N of the Group Output node', () => {
      expect(groupOutputIndex('out_0')).toBe(0);
      expect(groupOutputIndex('out_3')).toBe(3);
      expect(groupOutputIndex('out_12')).toBe(12);
    });

    it('is the first output for the `Out` socket of older groups', () => {
      expect(groupOutputIndex('Out')).toBe(0);
    });
  });

  describe('groupInputValue', () => {
    it('is the value set on the instance', () => {
      expect(groupInputValue(instance({ input_0: 0.25 }), group, 'input_0')).toBe(0.25);
      expect(groupInputValue(instance({ input_0: 0 }), group, 'input_0')).toBe(0);
    });

    it('is the default of the group input while the instance has no value for it', () => {
      expect(groupInputValue(instance({ input_2: 1 }), group, 'input_0')).toBe(0.5);
      expect(groupInputValue(instance(), group, 'input_3')).toEqual([0, 1, 0]);
    });

    it('is undefined when neither has a value', () => {
      expect(groupInputValue(instance(), group, 'input_2')).toBeUndefined();
      expect(groupInputValue(instance({ groupId: 4 }), {}, 'input_0')).toBeUndefined();
    });
  });
});
