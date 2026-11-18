import { INTERNAL_PREFIX } from '@nodarium/types';

/** The system of a node type, `plantarium` of `max/plantarium/stem`; none for groups and other internal nodes */
export function nodeSystem(type: string): { collection: string; name: string } | undefined {
  if (type.startsWith(INTERNAL_PREFIX)) return;
  const parts = type.split('/');
  if (parts.length !== 3) return;
  return { collection: `${parts[0]}/${parts[1]}`, name: parts[1] };
}

/** The colour of the bar on the header, only for the collections the options give one */
export function systemColor(
  type: string,
  colors: Record<string, string> | undefined
): string | undefined {
  const system = nodeSystem(type);
  return system && colors?.[system.collection];
}
