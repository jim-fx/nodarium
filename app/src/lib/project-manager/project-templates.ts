import {
  agave,
  bamboo,
  champignonGroomed,
  chanterelle,
  chestnut,
  daisy,
  defaultPlant,
  devilsBackbone,
  fern,
  foxglove,
  grass,
  horsetail,
  knotweed,
  ladyFern,
  lavender,
  maidenhair,
  oyster,
  palm,
  sage,
  sapling,
  wildCarrot,
  willow
} from '#lib/graph-templates/index.ts';
import type { Graph, ProjectUnit } from '@nodarium/types';

export type TemplateCategory = 'Plants' | 'Mushrooms';

export type ProjectTemplate = {
  name: string;
  category: TemplateCategory;
  graph: Graph;
};

function toGraph(template: unknown, unit?: ProjectUnit): Graph {
  const graph = { id: 0, groups: [], ...(template as Partial<Graph>) } as Graph;
  return unit ? { ...graph, meta: { ...graph.meta, unit } } : graph;
}

const plant = (name: string, graph: Graph): ProjectTemplate => ({
  name,
  category: 'Plants',
  graph
});
const mushroom = (name: string, graph: Graph): ProjectTemplate => ({
  name,
  category: 'Mushrooms',
  graph
});

export const templateCategories: TemplateCategory[] = ['Plants', 'Mushrooms'];

export const projectTemplates: ProjectTemplate[] = [
  plant('Default Plant', toGraph(defaultPlant)),
  plant('Fern', toGraph(fern, 'cm')),
  plant('Weeping Willow', toGraph(willow)),
  plant('Agave', toGraph(agave)),
  plant('Ornamental Grass', toGraph(grass, 'cm')),
  plant('Palm', toGraph(palm)),
  plant('Sapling', toGraph(sapling)),
  plant('Lavender', toGraph(lavender, 'cm')),
  plant('Sage', toGraph(sage, 'cm')),
  plant('Bamboo', toGraph(bamboo)),
  plant('Lady Fern', toGraph(ladyFern, 'cm')),
  plant('Maidenhair Fern', toGraph(maidenhair, 'cm')),
  plant('Daisy', toGraph(daisy, 'cm')),
  plant('Field Horsetail', toGraph(horsetail, 'cm')),
  plant('Foxglove', toGraph(foxglove)),
  plant("Devil's Backbone", toGraph(devilsBackbone, 'cm')),
  plant('Japanese Knotweed', toGraph(knotweed)),
  plant('Wild Carrot', toGraph(wildCarrot, 'cm')),
  mushroom('Champignon', toGraph(champignonGroomed, 'cm')),
  mushroom('Chestnut mushroom', toGraph(chestnut, 'cm')),
  mushroom('Chanterelle', toGraph(chanterelle, 'cm')),
  mushroom('Oyster mushroom', toGraph(oyster, 'cm'))
];
