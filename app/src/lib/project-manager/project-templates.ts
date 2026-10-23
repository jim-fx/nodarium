import {
  agave,
  bamboo,
  banana,
  daisy,
  defaultPlant,
  fern,
  foxglove,
  grass,
  horsetail,
  ladyFern,
  lavender,
  maidenhair,
  palm,
  sage,
  sapling,
  willow
} from '#lib/graph-templates/index.ts';
import type { Graph, ProjectUnit } from '@nodarium/types';

export type ProjectTemplate = {
  name: string;
  graph: Graph;
};

function toGraph(template: unknown, unit?: ProjectUnit): Graph {
  const graph = { id: 0, groups: [], ...(template as Partial<Graph>) } as Graph;
  return unit ? { ...graph, meta: { ...graph.meta, unit } } : graph;
}

export const projectTemplates: ProjectTemplate[] = [
  { name: 'Default Plant', graph: toGraph(defaultPlant) },
  { name: 'Fern', graph: toGraph(fern, 'cm') },
  { name: 'Weeping Willow', graph: toGraph(willow) },
  { name: 'Agave', graph: toGraph(agave) },
  { name: 'Ornamental Grass', graph: toGraph(grass, 'cm') },
  { name: 'Palm', graph: toGraph(palm) },
  { name: 'Sapling', graph: toGraph(sapling) },
  { name: 'Lavender', graph: toGraph(lavender, 'cm') },
  { name: 'Sage', graph: toGraph(sage, 'cm') },
  { name: 'Bamboo', graph: toGraph(bamboo) },
  { name: 'Lady Fern', graph: toGraph(ladyFern, 'cm') },
  { name: 'Maidenhair Fern', graph: toGraph(maidenhair, 'cm') },
  { name: 'Daisy', graph: toGraph(daisy, 'cm') },
  { name: 'Field Horsetail', graph: toGraph(horsetail, 'cm') },
  { name: 'Foxglove', graph: toGraph(foxglove) },
  { name: 'Banana', graph: toGraph(banana) }
];
