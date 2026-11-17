import { getOutputType, type NodeDefinition, type NodeInput } from '@nodarium/types';

/** The input types that carry data from another node, the rest are values set on the node */
const DATA_TYPES = new Set(['path', 'geometry', 'force', 'points', 'mesh', 'pattern', 'shape']);

/** The first sentence of a description, a sentence ends with a stop before a capital */
export function firstSentence(text: string) {
  return text.match(/^(.*?[.!?])(?=\s+[A-Z]|$)/s)?.[1] ?? text;
}

/** A node in short: the types of the data it takes and gives, the first sentence of what it does */
export function nodeSummary(node: NodeDefinition) {
  const inputs = Object.values(node.inputs ?? {})
    .filter((input) => DATA_TYPES.has(input.type) && !input.hidden && !input.internal)
    .map((input) => input.type);
  const outputs = (node.outputs ?? []).map(getOutputType);
  const description = node.meta?.description && firstSentence(node.meta.description);
  return { id: node.id, inputs, outputs, ...(description && { description }) };
}

/** `max/plantarium/noise (path) -> path`: the id, the data the node takes and what it gives */
export function nodeSignature(node: NodeDefinition) {
  const { inputs, outputs } = nodeSummary(node);
  return `${node.id} (${inputs.join(', ')})${outputs.length ? ` -> ${outputs.join(', ')}` : ''}`;
}

function formatValue(input: NodeInput) {
  const value = input.value as unknown;
  if (input.type === 'select') {
    const names = (input.options ?? []).map((o) => (typeof o === 'string' ? o : o.label));
    const selected = typeof value === 'string'
      ? value
      : names[typeof value === 'number' ? value : 0];
    return selected === undefined ? '' : ` = ${selected} (${names.join(', ')})`;
  }
  if (DATA_TYPES.has(input.type) || value === undefined) return '';
  if (Array.isArray(value)) {
    return value.length <= 4 ? ` = [${value.join(', ')}]` : ` = [${value.length} numbers]`;
  }
  return ` = ${String(value)}`;
}

/** One input as `scale: float = 1 [0.1..10]`, with its unit and how it is set */
function formatInput(key: string, input: NodeInput) {
  const types = [...new Set([input.type, ...(input.accepts ?? [])])].join(' | ');
  const range = 'min' in input || 'max' in input
    ? ` [${input.min ?? ''}..${input.max ?? ''}]`
    : '';
  const unit = 'unit' in input && input.unit ? ` ${input.unit}` : '';
  const notes = [
    input.hidden && 'hidden',
    input.internal && 'not connectable',
    input.setting && `setting ${input.setting}`
  ].filter(Boolean);
  const value = formatValue(input);
  const withUnit = value && unit ? `${value}${unit}` : value;
  return `${key}: ${types}${withUnit}${range}${notes.length ? ` (${notes.join(', ')})` : ''}`;
}

/**
 * A node as an agent reads it: the signature, the description, and an input per line. With
 * `descriptions` every input is followed by its description.
 */
export function formatNode(node: NodeDefinition, { descriptions = true } = {}) {
  const lines = [nodeSignature(node)];
  if (node.meta?.description) lines.push(`  ${node.meta.description}`);
  for (const [key, input] of Object.entries(node.inputs ?? {})) {
    if (input.type === 'creator') continue;
    lines.push(`  ${formatInput(key, input)}`);
    if (descriptions && input.description) lines.push(`    ${input.description}`);
  }
  return lines.join('\n');
}

/** The list of the nodes: the signature and the first sentence of the description of each */
export function formatNodeList(nodes: NodeDefinition[]) {
  return nodes
    .map((node) => {
      const description = node.meta?.description;
      return description
        ? `${nodeSignature(node)}\n  ${firstSentence(description)}`
        : nodeSignature(node);
    })
    .join('\n');
}

/** The nodes `name` means: an id, `collection/node` or the name of the node alone */
export function findNodes(nodes: NodeDefinition[], name: string) {
  const exact = nodes.filter((node) => node.id === name);
  if (exact.length) return exact;
  return nodes.filter((node) => node.id.endsWith(`/${name}`));
}

/** The nodes that take a `type` (as its type or one it accepts) or give one */
export function nodesOfType(nodes: NodeDefinition[], type: string) {
  return nodes.filter((node) =>
    Object.values(node.inputs ?? {}).some((input) =>
      input.type === type || (input.accepts as string[] | undefined)?.includes(type)
    ) || (node.outputs ?? []).some((output) => getOutputType(output) === type)
  );
}

/** The nodes whose id or description contains `query`, ignoring case */
export function searchNodes(nodes: NodeDefinition[], query: string) {
  const word = query.toLowerCase();
  return nodes.filter((node) =>
    node.id.toLowerCase().includes(word)
    || node.meta?.description?.toLowerCase().includes(word)
  );
}
