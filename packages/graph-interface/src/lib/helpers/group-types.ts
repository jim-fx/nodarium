import {
  GROUP_INPUT,
  GROUP_INSTANCE,
  GROUP_OUTPUT,
  type GroupDefinition,
  type NodeDefinition,
  type NodeId,
  type NodeValue
} from '@nodarium/runtime';

export type GroupOption = { value: number; label: string };

const passFirstInput = (inputs: NodeValue[]) => inputs[0];

/**
 * The Group Input node of an open group. Its inputs are the output sockets of the node:
 * socket N stands for the N-th input of the group.
 */
export function groupInputType(group: GroupDefinition): NodeDefinition {
  return {
    id: GROUP_INPUT as NodeId,
    meta: { title: 'Group Input' },
    inputs: Object.fromEntries(
      Object.values(group.inputs ?? {}).map((input, i) => [`in_${i}`, { ...input, external: true }])
    ),
    execute: passFirstInput
  } as NodeDefinition;
}

/** The Group Output node of an open group, input `out_N` is output N of the group */
export function groupOutputType(group: GroupDefinition): NodeDefinition {
  return {
    id: GROUP_OUTPUT as NodeId,
    inputs: Object.fromEntries(
      (group.outputs ?? []).map((output, i) => [
        `out_${i}`,
        { type: output.type, label: output.label, external: true }
      ])
    ),
    meta: { title: 'Group Output' },
    outputs: [],
    execute: passFirstInput
  } as NodeDefinition;
}

/**
 * The groups an instance can point to. A group on the path can't be placed inside itself,
 * groups with the same name are told apart by their id.
 */
export function groupOptions(groups: GroupDefinition[], onPath: Set<number>): GroupOption[] {
  const options = groups
    .filter((group) => !onPath.has(group.id))
    .map((group) => ({ value: group.id, label: group.name || `Group#${group.id}` }));

  const amounts = new Map<string, number>();
  for (const { label } of options) amounts.set(label, (amounts.get(label) ?? 0) + 1);

  return options.map((option) =>
    amounts.get(option.label)! > 1
      ? { value: option.value, label: `${option.label}#${option.value}` }
      : option
  );
}

/**
 * A group instance: the select of its group, then the inputs and outputs of the group.
 *
 * Not kept per group: the inputs of a group are edited in place while its instances are shown,
 * and a definition that stays the same object keeps the node height cached for it.
 */
export function groupInstanceType(
  base: NodeDefinition | undefined,
  group: GroupDefinition | undefined,
  options: GroupOption[]
): NodeDefinition {
  const select = { type: 'select', label: '', internal: true, options };
  return {
    ...base,
    id: GROUP_INSTANCE as NodeId,
    meta: { title: 'Group', ...base?.meta },
    inputs: group
      ? { groupId: { ...select, value: group.id }, ...group.inputs }
      : { groupId: select },
    outputs: group ? group.outputs : []
  } as NodeDefinition;
}

const exposedTypes = new WeakMap<NodeDefinition, Map<string, NodeDefinition>>();

/** The type with its exposed hidden inputs shown, one definition per set of exposed inputs */
export function exposeInputs(
  type: NodeDefinition | undefined,
  exposed?: string[]
): NodeDefinition | undefined {
  if (!type?.inputs || !exposed?.some((key) => type.inputs?.[key]?.hidden)) return type;

  const cacheKey = exposed.join(',');
  let cache = exposedTypes.get(type);
  if (!cache) {
    cache = new Map();
    exposedTypes.set(type, cache);
  }

  let exposedType = cache.get(cacheKey);
  if (!exposedType) {
    exposedType = {
      ...type,
      inputs: Object.fromEntries(
        Object.entries(type.inputs).map(([key, input]) => [
          key,
          exposed.includes(key) ? { ...input, hidden: false } : input
        ])
      )
    };
    cache.set(cacheKey, exposedType);
  }
  return exposedType;
}
