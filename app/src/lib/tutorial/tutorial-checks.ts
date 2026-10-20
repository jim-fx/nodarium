import type { Edge } from '@nodarium/types';

const STEM = 'max/plantarium/stem';
const NOISE = 'max/plantarium/noise';
const RANDOM = 'max/plantarium/random';
const OUTPUT = 'max/plantarium/output';

function connected(edges: Edge[], from: string, to: string, inputs?: string[]) {
  return edges.some(([fromNode, , toNode, input]) =>
    fromNode.type === from && toNode.type === to && (!inputs || inputs.includes(input))
  );
}

/** Conditions the "build your own plant" steps wait for before moving on */
export const tutorialChecks = {
  stemToOutput: (edges: Edge[]) => connected(edges, STEM, OUTPUT),
  noiseBetweenStemAndOutput: (edges: Edge[]) =>
    connected(edges, STEM, NOISE) && connected(edges, NOISE, OUTPUT),
  randomOnStem: (edges: Edge[]) => connected(edges, RANDOM, STEM, ['length', 'thickness'])
};
