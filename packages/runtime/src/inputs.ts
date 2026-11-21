import type { NodeInput, SerializedNode } from '@nodarium/types';
import { encodeFloat } from '@nodarium/utils';
import type { PlannedInput, PlanSource } from './plan';

/**
 * The id a node writes into the paths it makes, so every path knows which node made it.
 * Node ids can be larger than 32 bits (nodes of a group instance), the wasm side reads an i32.
 */
export function toCreatorId(nodeId: number): number {
  return (nodeId | 0) ^ (Math.floor(nodeId / 2 ** 32) | 0);
}

/** A text as the host sends it, one word per Unicode code point: `[0, n + 1, ...code points, 1, 1]` */
export function encodeText(text: string) {
  const codePoints = Array.from(text, (c) => c.codePointAt(0)!);
  return [0, codePoints.length + 1, ...codePoints, 1, 1];
}

export function getValue(input: NodeInput, value?: unknown) {
  if (value === undefined && 'value' in input) {
    value = input.value;
  }

  if (input.type === 'float') {
    return encodeFloat(value as number);
  }

  if (input.type === 'text') {
    return encodeText(String(value ?? ''));
  }

  if (input.type === 'select' && typeof value !== 'number') {
    const index = input.options?.indexOf(value as string);
    if (index === undefined || index < 0) {
      // Default to the first option
      return 0;
    }
    return index;
  }

  if (Array.isArray(value)) {
    if (input.type === 'vec3' || input.type === 'shape' || input.type === 'curve') {
      return [
        0,
        value.length + 1,
        ...value.map((v) => encodeFloat(v)),
        1,
        1
      ] as number[];
    }
    return [0, value.length + 1, ...value, 1, 1] as number[];
  }

  if (typeof value === 'boolean') {
    return value ? 1 : 0;
  }

  if (typeof value === 'number') {
    return value;
  }

  if (value instanceof Int32Array) {
    return value;
  }

  // an unconnected input without a default, like the geometry of the output node
  if (value === undefined) {
    return [0, 1, 1, 1];
  }

  throw new Error(`Unknown input type ${input.type}`);
}

export type InputContext = {
  seed: number;
  settings: Record<string, unknown>;
  /** the parts of the cache key of the node, every input adds one */
  keyParts: (string | number | undefined)[];
  /** inputs that fell back to an empty value, nodes may panic on these */
  missingInputs: string[];
};

/**
 * The value of an input comes from the first source it has of: a connected node, the seed, the
 * id of the node, a setting, the props of the node, the default of the input.
 */
export function resolveInput(
  node: { id: number; props?: SerializedNode['props'] },
  key: string,
  input: NodeInput,
  connection: { node: PlanSource; output: number } | undefined,
  ctx: InputContext
): PlannedInput {
  if (connection && !input.setting && input.type !== 'seed' && input.type !== 'creator') {
    const sourceKey = connection.node.cacheKey;
    ctx.keyParts.push(connection.output === 0 ? sourceKey : `${sourceKey}:${connection.output}`);
    return connection;
  }

  const value = unconnectedValue(node, key, input, ctx);
  if (typeof value === 'number') {
    ctx.keyParts.push('n' + value);
    return Int32Array.of(value);
  }
  ctx.keyParts.push('a' + value.join(','));
  return value instanceof Int32Array ? value : Int32Array.from(value);
}

function unconnectedValue(
  node: { id: number; props?: SerializedNode['props'] },
  key: string,
  input: NodeInput,
  ctx: InputContext
) {
  if (input.type === 'seed') return ctx.seed;
  if (input.type === 'creator') return toCreatorId(node.id);
  if (input.setting) return getValue(input, ctx.settings[input.setting]);
  if (node.props?.[key] !== undefined) return getValue(input, node.props[key]);
  if (!('value' in input) || input.value === undefined) ctx.missingInputs.push(key);
  return getValue(input);
}
