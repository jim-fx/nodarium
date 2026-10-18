import {
  agave,
  bamboo,
  daisy,
  defaultPlant,
  fern,
  grass,
  ladyFern,
  lavender,
  maidenhair,
  palm,
  sage,
  sapling,
  willow
} from '$lib/graph-templates';
import type { Graph } from '@nodarium/types';

export type ProjectTemplate = {
  name: string;
  graph: Graph;
};

function toGraph(template: unknown): Graph {
  return { id: 0, groups: [], ...(template as Partial<Graph>) } as Graph;
}

export const projectTemplates: ProjectTemplate[] = [
  { name: 'Default Plant', graph: toGraph(defaultPlant) },
  { name: 'Fern', graph: toGraph(fern) },
  { name: 'Weeping Willow', graph: toGraph(willow) },
  { name: 'Agave', graph: toGraph(agave) },
  { name: 'Ornamental Grass', graph: toGraph(grass) },
  { name: 'Palm', graph: toGraph(palm) },
  { name: 'Sapling', graph: toGraph(sapling) },
  { name: 'Lavender', graph: toGraph(lavender) },
  { name: 'Sage', graph: toGraph(sage) },
  { name: 'Bamboo', graph: toGraph(bamboo) },
  { name: 'Lady Fern', graph: toGraph(ladyFern) },
  { name: 'Maidenhair Fern', graph: toGraph(maidenhair) },
  { name: 'Daisy', graph: toGraph(daisy) }
];
