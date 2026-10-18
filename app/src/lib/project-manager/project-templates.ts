import {
  agave,
  bamboo,
  defaultPlant,
  fern,
  grass,
  lavender,
  lottaFaces,
  palm,
  plant,
  sapling,
  simple,
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
  { name: 'Plant', graph: toGraph(plant) },
  { name: 'Fern', graph: toGraph(fern) },
  { name: 'Weeping Willow', graph: toGraph(willow) },
  { name: 'Agave', graph: toGraph(agave) },
  { name: 'Ornamental Grass', graph: toGraph(grass) },
  { name: 'Palm', graph: toGraph(palm) },
  { name: 'Sapling', graph: toGraph(sapling) },
  { name: 'Lavender', graph: toGraph(lavender) },
  { name: 'Bamboo', graph: toGraph(bamboo) },
  { name: 'Simple', graph: toGraph(simple) },
  { name: 'Lotta Faces', graph: toGraph(lottaFaces) }
];
