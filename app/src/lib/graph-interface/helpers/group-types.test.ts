import { GROUP_INPUT, GROUP_INSTANCE, GROUP_OUTPUT } from '@nodarium/types';
import type { GroupDefinition, NodeDefinition } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import {
  exposeInputs,
  groupInputType,
  groupInstanceType,
  groupOptions,
  groupOutputType
} from './group-types';

const group = (id: number, name?: string): GroupDefinition => ({
  id,
  ...(name !== undefined && { name }),
  nodes: [],
  edges: []
});

const stipe: GroupDefinition = {
  id: 7,
  name: 'Stipe',
  nodes: [],
  edges: [],
  inputs: {
    length: { type: 'float', label: 'Length', value: 0.5 },
    // a removed input leaves a gap in the names, not in the sockets
    input_2: { type: 'path', label: 'Path' }
  },
  outputs: [{ type: 'path', label: 'Stipe' }, { type: 'float' }]
};

describe('groupOptions', () => {
  it('lists the groups by their name, a group without a name by its id', () => {
    expect(groupOptions([group(3, 'Cap'), group(4), group(5, '')], new Set())).toEqual([
      { value: 3, label: 'Cap' },
      { value: 4, label: 'Group#4' },
      { value: 5, label: 'Group#5' }
    ]);
  });

  it('tells groups with the same name apart by their id', () => {
    expect(groupOptions([group(3, 'Cap'), group(4, 'Gills'), group(5, 'Cap')], new Set())).toEqual(
      [
        { value: 3, label: 'Cap#3' },
        { value: 4, label: 'Gills' },
        { value: 5, label: 'Cap#5' }
      ]
    );
  });

  it('leaves out the groups on the path, they can not be placed inside themselves', () => {
    const groups = [group(3, 'Cap'), group(4, 'Gills'), group(5, 'Veil')];
    expect(groupOptions(groups, new Set([4, 3])).map((o) => o.value)).toEqual([5]);
  });

  it('does not count a group on the path as a second one of its name', () => {
    expect(groupOptions([group(3, 'Cap'), group(5, 'Cap')], new Set([3]))).toEqual([
      { value: 5, label: 'Cap' }
    ]);
  });

  it('is empty without groups', () => {
    expect(groupOptions([], new Set())).toEqual([]);
  });
});

describe('groupInputType', () => {
  it('has one socket per input of the group, in their order', () => {
    const type = groupInputType(stipe);
    expect(type.id).toBe(GROUP_INPUT);
    expect(type.inputs).toEqual({
      in_0: { type: 'float', label: 'Length', value: 0.5, external: true },
      in_1: { type: 'path', label: 'Path', external: true }
    });
  });

  it('has no sockets for a group without inputs', () => {
    expect(groupInputType(group(3)).inputs).toEqual({});
  });
});

describe('groupOutputType', () => {
  it('has the input out_N for output N of the group and no outputs', () => {
    const type = groupOutputType(stipe);
    expect(type.id).toBe(GROUP_OUTPUT);
    expect(type.inputs).toEqual({
      out_0: { type: 'path', label: 'Stipe', external: true },
      out_1: { type: 'float', external: true }
    });
    expect(type.outputs).toEqual([]);
  });

  it('has no inputs for a group without outputs', () => {
    expect(groupOutputType(group(3)).inputs).toEqual({});
  });
});

describe('groupInstanceType', () => {
  const base = {
    id: GROUP_INSTANCE,
    meta: { title: 'Group', description: 'A group of nodes' },
    inputs: {},
    outputs: []
  } as unknown as NodeDefinition;
  const options = [{ value: 7, label: 'Stipe' }, { value: 9, label: 'Cap' }];

  it('shows the select of its group, then the inputs of the group, and its outputs', () => {
    const type = groupInstanceType(base, stipe, options);
    expect(type.id).toBe(GROUP_INSTANCE);
    expect(Object.keys(type.inputs!)).toEqual(['groupId', 'length', 'input_2']);
    expect(type.inputs!.groupId).toEqual({
      type: 'select',
      label: '',
      internal: true,
      value: 7,
      options
    });
    expect(type.inputs!.length).toEqual(stipe.inputs!.length);
    expect(type.outputs).toEqual(stipe.outputs);
    expect(type.meta).toEqual(base.meta);
  });

  it('only has the select while it points to no group', () => {
    const type = groupInstanceType(base, undefined, options);
    expect(type.inputs).toEqual({
      groupId: { type: 'select', label: '', internal: true, options }
    });
    expect(type.outputs).toEqual([]);
  });

  it('works without a type in the registry', () => {
    const type = groupInstanceType(undefined, stipe, options);
    expect(type.id).toBe(GROUP_INSTANCE);
    expect(type.meta).toEqual({ title: 'Group' });
    expect(Object.keys(type.inputs!)).toEqual(['groupId', 'length', 'input_2']);
  });

  it('leaves the registry type and the group as they were', () => {
    const before = structuredClone({ base, stipe });
    groupInstanceType(base, stipe, options);
    expect({ base, stipe }).toEqual(before);
  });
});

describe('exposeInputs', () => {
  const type = {
    id: 'test/node/hidden',
    inputs: {
      shown: { type: 'float' },
      amount: { type: 'float', hidden: true },
      clip: { type: 'boolean', hidden: true }
    },
    outputs: ['float']
  } as unknown as NodeDefinition;

  it('returns the type itself while none of its hidden inputs is exposed', () => {
    expect(exposeInputs(type)).toBe(type);
    expect(exposeInputs(type, [])).toBe(type);
    expect(exposeInputs(type, ['shown', 'unknown'])).toBe(type);
    expect(exposeInputs(undefined, ['amount'])).toBeUndefined();
  });

  it('shows the exposed inputs and keeps the others hidden', () => {
    const exposed = exposeInputs(type, ['amount']);
    expect(exposed?.inputs?.amount.hidden).toBe(false);
    expect(exposed?.inputs?.clip.hidden).toBe(true);
    expect(exposed?.inputs?.shown).toBe(type.inputs!.shown);
    expect(Object.keys(exposed!.inputs!)).toEqual(['shown', 'amount', 'clip']);
    expect(type.inputs!.amount.hidden).toBe(true);
  });

  it('gives every node with the same exposed inputs the same definition', () => {
    expect(exposeInputs(type, ['amount'])).toBe(exposeInputs(type, ['amount']));
    expect(exposeInputs(type, ['amount'])).not.toBe(exposeInputs(type, ['amount', 'clip']));
  });
});
