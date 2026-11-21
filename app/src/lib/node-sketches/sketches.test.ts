import { readdirSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  NODE_SKETCHES,
  SKETCH_PERIOD,
  sketchFor,
  type SketchFrame,
  sketchPhase,
  UNSKETCHED
} from './sketches';

function coordinates(frame: SketchFrame) {
  return [...frame.input, ...frame.output].flatMap((shape) =>
    shape.kind === 'dot' ? shape.at : shape.points.flat()
  );
}

const NODES_DIR = new URL('../../../../nodes/max/', import.meta.url);
const nodeIds = ['core', 'plantarium', 'myzel'].flatMap((collection) =>
  readdirSync(new URL(`${collection}/`, NODES_DIR), { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith('.'))
    .map((entry) => `max/${collection}/${entry.name}`)
);

describe('sketchFor', () => {
  it('every node has a sketch or a reason why it has none, never both', () => {
    expect(nodeIds.length).toBeGreaterThan(50);
    for (const id of nodeIds) {
      expect(id in NODE_SKETCHES !== id in UNSKETCHED, id).toBe(true);
    }
    for (const id of [...Object.keys(NODE_SKETCHES), ...Object.keys(UNSKETCHED)]) {
      expect(nodeIds, id).toContain(id);
    }
  });

  it('finds the sketch of a node, nothing for a group', () => {
    expect(sketchFor('max/plantarium/branch')).toBe(NODE_SKETCHES['max/plantarium/branch']);
    expect(sketchFor('max/core/output')).toBeUndefined();
    expect(sketchFor('__internal/group/instance')).toBeUndefined();
  });
});

describe('the sketches', () => {
  for (const [id, sketch] of Object.entries(NODE_SKETCHES)) {
    it(`${id} stays inside its panels and changes from start to end`, () => {
      for (const p of [0, 0.25, 0.5, 1]) {
        const frame = sketch.draw(p, 3);
        expect(frame.input.length > 0).toBe(sketch.input !== undefined);
        expect(frame.output.length).toBeGreaterThan(0);
        for (const c of coordinates(frame)) {
          expect(Number.isFinite(c)).toBe(true);
          expect(c).toBeGreaterThanOrEqual(0);
          expect(c).toBeLessThanOrEqual(1);
        }
      }
      expect(coordinates(sketch.draw(0, 0))).not.toEqual(coordinates(sketch.draw(1, 0)));
    });
  }
});

describe('sketchPhase', () => {
  it('goes up, rests at the end, goes back and rests at the start', () => {
    expect(sketchPhase(0).p).toBe(0);
    expect(sketchPhase(SKETCH_PERIOD * 0.5).p).toBe(1);
    expect(sketchPhase(SKETCH_PERIOD * 0.9).p).toBe(0);
    const up = sketchPhase(SKETCH_PERIOD * 0.2).p;
    expect(up).toBeGreaterThan(0);
    expect(up).toBeLessThan(1);
  });

  it('counts the loops', () => {
    expect(sketchPhase(SKETCH_PERIOD * 2.5).cycle).toBe(2);
  });
});
