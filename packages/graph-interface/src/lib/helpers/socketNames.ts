import {
  getOutputLabel,
  GROUP_INPUT,
  type NodeDefinition,
  type NodeInstance
} from '@nodarium/runtime';

// Names for assistive technology and agents that read the page, built from what the node shows

type NamedNode = Pick<NodeInstance, 'id' | 'type'>;

/** The title in the header of the node */
export function nodeTitle(node: NamedNode, type: NodeDefinition | undefined): string {
  return type?.meta?.title || node.type.split('/').pop() || node.type;
}

/** "stem, node 10" */
export function nodeLabel(node: NamedNode, type: NodeDefinition | undefined): string {
  return `${nodeTitle(node, type)}, node ${node.id}`;
}

/** The label of an input row, as the node shows it */
export function inputLabel(type: NodeDefinition | undefined, key: string): string {
  return type?.inputs?.[key]?.label || key;
}

/** The label of output `index`. The outputs of the Group Input node are its rows, see getNodeSockets() */
export function outputLabel(
  node: NamedNode,
  type: NodeDefinition | undefined,
  index: number
): string {
  if (node.type === GROUP_INPUT) {
    const key = Object.keys(type?.inputs ?? {})[index];
    return key === undefined ? String(index) : inputLabel(type, key);
  }
  return getOutputLabel(type?.outputs?.[index]) ?? String(index);
}

/** "output path of stem" for output 0 of a stem, "input plant of noise" for the input "plant" of a noise */
export function socketName(
  node: NamedNode,
  type: NodeDefinition | undefined,
  index: number | string
): string {
  const title = nodeTitle(node, type);
  return typeof index === 'number'
    ? `output ${outputLabel(node, type, index)} of ${title}`
    : `input ${inputLabel(type, index)} of ${title}`;
}

/** "connected to output path of stem (#10)": what feeds an input */
export function connectionDescription(
  from: NamedNode,
  fromType: NodeDefinition | undefined,
  fromIndex: number
): string {
  return `connected to ${socketName(from, fromType, fromIndex)} (#${from.id})`;
}
