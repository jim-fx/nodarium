import type { Box, Frame } from '@nodarium/runtime';

/** Around the nodes of a frame, in graph units */
export const FRAME_MARGIN = 5;
/** Above the nodes of a frame, room for its title */
export const FRAME_HEADER = 9;

/** The rectangle of a frame around the boxes of its nodes, undefined without nodes */
export function frameBox(boxes: Iterable<Box>): Box | undefined {
  let box: Box | undefined;
  for (const b of boxes) {
    box = box
      ? {
        minX: Math.min(box.minX, b.minX),
        maxX: Math.max(box.maxX, b.maxX),
        minY: Math.min(box.minY, b.minY),
        maxY: Math.max(box.maxY, b.maxY)
      }
      : { ...b };
  }
  if (!box) return;
  return {
    minX: box.minX - FRAME_MARGIN,
    maxX: box.maxX + FRAME_MARGIN,
    minY: box.minY - FRAME_MARGIN - FRAME_HEADER,
    maxY: box.maxY + FRAME_MARGIN
  };
}

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
