import { describe, expect, it } from 'vitest';
import { NODE_SKETCHES, SKETCH_PERIOD, sketchFor, type SketchFrame, sketchPhase } from './sketches';

function coordinates(frame: SketchFrame) {
  return [...frame.input, ...frame.output].flatMap((shape) =>
    shape.kind === 'line' ? shape.points.flat() : shape.at
  );
}

describe('sketchFor', () => {
  it('gives a sketch to the five nodes that are hard to guess, and none to the rest', () => {
    expect(Object.keys(NODE_SKETCHES).sort()).toEqual([
      'max/core/alpha',
      'max/core/curve',
      'max/core/random',
      'max/myzel/strands',
      'max/plantarium/branch'
    ]);
    expect(sketchFor('max/plantarium/branch')).toBe(NODE_SKETCHES['max/plantarium/branch']);
    expect(sketchFor('max/plantarium/gravity')).toBeUndefined();
    expect(sketchFor('__internal/group/instance')).toBeUndefined();
  });
});

describe('the sketches', () => {
  for (const [id, sketch] of Object.entries(NODE_SKETCHES)) {
    it(`${id} stays inside its panels and changes from start to end`, () => {
      for (const p of [0, 0.25, 0.5, 1]) {
        const frame = sketch.draw(p, 3);
        expect(frame.input.length).toBeGreaterThan(0);
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

  it('random draws new values every time round', () => {
    const random = NODE_SKETCHES['max/core/random'];
    expect(random.draw(1, 0).output).not.toEqual(random.draw(1, 1).output);
    expect(random.draw(1, 4).output).toEqual(random.draw(1, 4).output);
  });
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
