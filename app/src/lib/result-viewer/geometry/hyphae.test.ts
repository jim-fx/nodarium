import { Group, LineSegments, MeshBasicMaterial } from 'three';
import { describe, expect, it } from 'vitest';
import { createResultPools } from './geometryPool';
import { hyphaeLines, readHyphae } from './hyphae';
import { encodeHyphae } from './hyphae-fixtures';

const sample = () =>
  encodeHyphae([
    { end: [0, 0, 0], parent: -1 },
    { end: [0, 1, 0], parent: 0, tissue: 1 },
    { end: [1, 2, 0], parent: 1, tissue: 5, birth: 2.5 },
    // torn off its parent
    { end: [1, 3, 0], parent: 2, tissue: 4, flags: 1 << 3 }
  ]);

describe('readHyphae', () => {
  it('starts every segment at the end of its parent', () => {
    const segments = readHyphae(sample());
    expect(segments).toHaveLength(4);
    expect(segments[0].start).toEqual(segments[0].end);
    expect(segments[1].start).toEqual([0, 0, 0]);
    expect(segments[2]).toMatchObject({ start: [0, 1, 0], end: [1, 2, 0], tissue: 'cap cuticle' });
    expect(segments[2].birth).toBe(2.5);
    expect(segments[1].radius).toBeCloseTo(0.001);
  });

  it('cuts broken segments loose', () => {
    const broken = readHyphae(sample())[3];
    expect(broken.tissue).toBe('veil');
    expect(broken.start).toEqual(broken.end);
  });

  it('rejects children before their parents and data of the wrong length', () => {
    expect(() => readHyphae(encodeHyphae([{ end: [0, 0, 0], parent: 0 }]))).toThrow(
      /parents have to come first/
    );
    expect(() => readHyphae(sample().subarray(0, 9))).toThrow(/need 34 words/);
  });
});

describe('hyphaeLines', () => {
  it('draws a line per grown segment, colored by tissue', () => {
    const { positions, colors } = hyphaeLines([sample()]);
    // the root and the broken segment have no line
    expect(positions).toEqual([0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 2, 0]);
    expect(colors).toHaveLength(12);
    // stipe and cap cuticle differ
    expect(colors.slice(0, 3)).not.toEqual(colors.slice(6, 9));
  });
});

describe('createResultPools with hyphae', () => {
  it('shows hyphae as lines and reports their bounds', () => {
    const group = new Group();
    const pools = createResultPools(group, new MeshBasicMaterial());
    const stats = pools.update([sample()]);
    expect(stats.bounds.max.toArray()).toEqual([1, 2, 0]);
    const lines: LineSegments[] = [];
    group.traverse((child) => {
      if (child instanceof LineSegments) lines.push(child);
    });
    expect(lines).toHaveLength(1);
    expect(lines[0].geometry.getAttribute('position').count).toBe(4);

    pools.update([]);
    const left: LineSegments[] = [];
    group.traverse((child) => {
      if (child instanceof LineSegments) left.push(child);
    });
    expect(left).toHaveLength(0);
  });
});
