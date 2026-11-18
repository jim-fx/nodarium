import { NO_PATH } from '@nodarium/utils';
import { describe, expect, it } from 'vitest';
import {
  createGroupModes,
  HIGHLIGHT,
  type HighlightMode,
  MAX_GROUPS,
  type PartHighlight,
  setGroupModes,
  visibleParts
} from './highlight';
import { partHit } from './part-hit';
import { decodePixel } from './picker';

function highlight(
  creators: [number, HighlightMode][],
  rest: HighlightMode = HIGHLIGHT.none
): PartHighlight {
  return { paths: new Map(), instances: new Map(), creators: new Map(creators), rest };
}

describe('setGroupModes', () => {
  it('gives every group the mode of the node that made it', () => {
    const modes = createGroupModes();
    setGroupModes(
      modes,
      Int32Array.of(40, 41, 42),
      highlight([[41, HIGHLIGHT.hover], [42, HIGHLIGHT.focus]], HIGHLIGHT.hidden)
    );
    expect([...modes.value.subarray(0, 3)]).toEqual([
      HIGHLIGHT.hidden,
      HIGHLIGHT.hover,
      HIGHLIGHT.focus
    ]);
    // groups past the table share the mode of its last one
    expect(modes.value[MAX_GROUPS - 1]).toBe(HIGHLIGHT.focus);
  });

  it('shows every group as it is without a highlight or a table', () => {
    const modes = createGroupModes();
    setGroupModes(modes, Int32Array.of(40), highlight([[40, HIGHLIGHT.hover]]));
    setGroupModes(modes, Int32Array.of(40), undefined);
    expect(modes.value.every((mode) => mode === HIGHLIGHT.none)).toBe(true);
    // the local view hides a mesh that can't tell its groups apart
    setGroupModes(modes, undefined, highlight([], HIGHLIGHT.hidden));
    expect(modes.value.every((mode) => mode === HIGHLIGHT.hidden)).toBe(true);
  });

  it('keeps the modes of two meshes apart', () => {
    const [a, b] = [createGroupModes(), createGroupModes()];
    const shown = highlight([[40, HIGHLIGHT.hover]]);
    setGroupModes(a, Int32Array.of(40, 50), shown);
    setGroupModes(b, Int32Array.of(50, 40), shown);
    expect([a.value[0], a.value[1]]).toEqual([HIGHLIGHT.hover, HIGHLIGHT.none]);
    expect([b.value[0], b.value[1]]).toEqual([HIGHLIGHT.none, HIGHLIGHT.hover]);
  });
});

describe('visibleParts', () => {
  it('leaves out what is hidden or dimmed', () => {
    const shown: PartHighlight = {
      paths: new Map([[1, HIGHLIGHT.none], [2, HIGHLIGHT.hidden]]),
      instances: new Map([[1, HIGHLIGHT.dimmed], [3, HIGHLIGHT.hover]]),
      creators: new Map([[40, HIGHLIGHT.focus]]),
      rest: HIGHLIGHT.hidden
    };
    const visible = visibleParts(shown);
    expect([...visible.paths]).toEqual([1]);
    expect([...visible.instances]).toEqual([3]);
    expect([...visible.creators]).toEqual([40]);
  });
});

describe('decodePixel', () => {
  it('reads the slot and the index from an integer pixel', () => {
    expect(decodePixel('integer', Uint32Array.of(3, 70000, 0, 1))).toEqual({
      slot: 3,
      index: 70000
    });
    expect(decodePixel('integer', new Uint32Array(4)).slot).toBe(0);
  });

  it('reads 16 bits of each from the bytes the fallback packs them into', () => {
    // slot 0x0102, index 0x0304, as the shader writes them: low byte first
    expect(decodePixel('bytes', Uint8Array.of(0x02, 0x01, 0x04, 0x03))).toEqual({
      slot: 0x0102,
      index: 0x0304
    });
  });
});

describe('partHit', () => {
  it('is the path of the geometry, or of the instance', () => {
    expect(partHit('geometry', { pathId: 7 }, 0)).toEqual({ kind: 'geometry', pathId: 7 });
    expect(partHit('instances', { pathIds: [5, 6, 6] }, 2)).toEqual({
      kind: 'instances',
      pathId: 6
    });
    expect(partHit('geometry', {}, 0).pathId).toBe(NO_PATH);
  });

  it('is the group of a Myzel mesh and the node that made it, when the mesh knows it', () => {
    expect(partHit('groups', { creators: Int32Array.of(40, 41) }, 1)).toEqual({
      kind: 'groups',
      pathId: NO_PATH,
      group: 1,
      creator: 41
    });
    expect(partHit('groups', {}, 1).creator).toBeUndefined();
  });
});
