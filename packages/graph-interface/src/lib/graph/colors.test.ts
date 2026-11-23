import { describe, expect, it } from 'vitest';
import { ColorGenerator, LINK_COLORS } from './colors';

const TYPES = Object.keys(LINK_COLORS);
const hsl = (type: string) => {
  const { hue, saturation, lightness } = LINK_COLORS[type];
  return `hsl(${hue}, ${saturation}%, ${lightness}%)`;
};

describe('ColorGenerator', () => {
  it('gives every link type its fixed colour whatever is drawn first', () => {
    for (const order of [TYPES, [...TYPES].reverse(), ['unknown', 'other', ...TYPES]]) {
      const colors = new ColorGenerator();
      const drawn = Object.fromEntries(order.map(t => [t, colors.getColor(t)]));
      for (const type of TYPES) expect(drawn[type]).toBe(hsl(type));
    }
  });

  it('gives the known types distinct colours', () => {
    const colors = new ColorGenerator();
    expect(new Set(TYPES.map(t => colors.getColor(t))).size).toBe(TYPES.length);
  });

  it('keeps the colour of an unknown type independent of the order', () => {
    const a = new ColorGenerator();
    a.getColor('alpha');
    const b = new ColorGenerator();
    expect(a.getColor('beta')).toBe(b.getColor('beta'));
  });
});
