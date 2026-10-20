import { DialogRunner } from '@nodarium/planty';
import type { Edge, NodeInstance } from '@nodarium/types';
import { describe, expect, it } from 'vitest';
import { tutorialChecks } from './tutorial-checks';
import { tutorialConfig } from './tutorial-config';

function node(type: string) {
  return { type } as NodeInstance;
}

const stem = node('max/plantarium/stem');
const noise = node('max/plantarium/noise');
const random = node('max/plantarium/random');
const output = node('max/plantarium/output');

describe('tutorialConfig', () => {
  it('has no dangling links or unreachable nodes', () => {
    expect(new DialogRunner(tutorialConfig).validate()).toEqual([]);
  });
});

describe('tutorialChecks', () => {
  it('needs the stem connected to the output', () => {
    expect(tutorialChecks.stemToOutput([])).toBe(false);
    expect(tutorialChecks.stemToOutput([[stem, 0, noise, 'plant']] as Edge[])).toBe(false);
    expect(tutorialChecks.stemToOutput([[stem, 0, output, 'input']] as Edge[])).toBe(true);
  });

  it('needs the noise between stem and output', () => {
    expect(
      tutorialChecks.noiseBetweenStemAndOutput([
        [stem, 0, output, 'input'],
        [stem, 0, noise, 'plant']
      ] as Edge[])
    ).toBe(false);
    expect(
      tutorialChecks.noiseBetweenStemAndOutput([
        [stem, 0, noise, 'plant'],
        [noise, 0, output, 'input']
      ] as Edge[])
    ).toBe(true);
  });

  it('needs the random node on the stem length or thickness', () => {
    expect(tutorialChecks.randomOnStem([[random, 0, stem, 'amount']] as Edge[])).toBe(false);
    expect(tutorialChecks.randomOnStem([[random, 0, stem, 'length']] as Edge[])).toBe(true);
    expect(tutorialChecks.randomOnStem([[random, 0, stem, 'thickness']] as Edge[])).toBe(true);
  });
});
