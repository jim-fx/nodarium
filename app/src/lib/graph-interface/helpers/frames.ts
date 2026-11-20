import type { Frame } from '@nodarium/types';

/**
 * The frames with only the nodes of `has`, each node in the first frame that names it, and
 * without the frames that are left empty. Returns `frames` itself when nothing changed.
 */
export function cleanFrames(frames: Frame[], has: (nodeId: number) => boolean): Frame[] {
  const seen = new Set<number>();
  let changed = false;
  const cleaned: Frame[] = [];
  for (const frame of frames) {
    const nodes = frame.nodes.filter((id) => {
      if (!has(id) || seen.has(id)) return false;
      seen.add(id);
      return true;
    });
    if (nodes.length !== frame.nodes.length) changed = true;
    if (!nodes.length) continue;
    cleaned.push(nodes.length === frame.nodes.length ? frame : { ...frame, nodes });
  }
  return changed ? cleaned : frames;
}

export function serializeFrame(frame: Frame): Frame {
  return {
    id: frame.id,
    label: frame.label,
    ...(frame.color !== undefined ? { color: frame.color } : {}),
    nodes: [...frame.nodes]
  };
}
