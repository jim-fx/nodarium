import type { Graph, ProjectUnit } from '@nodarium/types';

export type TemplateCategory = 'Plants' | 'Mushrooms';

export type Template = {
  /** The file name of the template, it is imported on its own from `@nodarium/runtime/templates/<id>` */
  id: string;
  name: string;
  category: TemplateCategory;
  /** The unit the app shows lengths in, the graph stores meters either way */
  unit: ProjectUnit;
  /** The graph is shared by everyone who loads it, clone it before changing it */
  load: () => Promise<Graph>;
};

export const templateCategories: TemplateCategory[] = ['Plants', 'Mushrooms'];

// every template is imported on its own, so a bundler splits them into chunks of their own
function template(category: TemplateCategory) {
  type Load = () => Promise<{ default: unknown }>;
  return (id: string, name: string, unit: ProjectUnit, load: Load): Template => ({
    id,
    name,
    category,
    unit,
    load: () => load().then((m) => m.default as Graph)
  });
}

const plant = template('Plants');
const mushroom = template('Mushrooms');

/** In the order of the project browser of the app */
export const templates: Template[] = [
  plant('default-plant', 'Default Plant', 'm', () => import('./default-plant.json')),
  plant('fern', 'Fern', 'cm', () => import('./fern.json')),
  plant('willow', 'Weeping Willow', 'm', () => import('./willow.json')),
  plant('agave', 'Agave', 'm', () => import('./agave.json')),
  plant('grass', 'Ornamental Grass', 'cm', () => import('./grass.json')),
  plant('palm', 'Palm', 'm', () => import('./palm.json')),
  plant('sapling', 'Sapling', 'm', () => import('./sapling.json')),
  plant('lavender', 'Lavender', 'cm', () => import('./lavender.json')),
  plant('sage', 'Sage', 'cm', () => import('./sage.json')),
  plant('bamboo', 'Bamboo', 'm', () => import('./bamboo.json')),
  plant('lady-fern', 'Lady Fern', 'cm', () => import('./lady-fern.json')),
  plant('maidenhair', 'Maidenhair Fern', 'cm', () => import('./maidenhair.json')),
  plant('daisy', 'Daisy', 'cm', () => import('./daisy.json')),
  plant('horsetail', 'Field Horsetail', 'cm', () => import('./horsetail.json')),
  plant('foxglove', 'Foxglove', 'm', () => import('./foxglove.json')),
  plant('devils-backbone', "Devil's Backbone", 'cm', () => import('./devils-backbone.json')),
  plant('knotweed', 'Japanese Knotweed', 'm', () => import('./knotweed.json')),
  plant('wild-carrot', 'Wild Carrot', 'cm', () => import('./wild-carrot.json')),
  mushroom('champignon-groomed', 'Champignon', 'cm', () => import('./champignon-groomed.json')),
  mushroom('chestnut', 'Chestnut mushroom', 'cm', () => import('./chestnut.json')),
  mushroom('fly-agaric', 'Fly agaric', 'cm', () => import('./fly-agaric.json')),
  mushroom('porcini', 'Porcini', 'cm', () => import('./porcini.json')),
  mushroom('shiitake', 'Shiitake', 'cm', () => import('./shiitake.json')),
  mushroom('shaggy-inkcap', 'Shaggy inkcap', 'cm', () => import('./shaggy-inkcap.json')),
  mushroom('chanterelle', 'Chanterelle', 'cm', () => import('./chanterelle.json')),
  mushroom('oyster', 'Oyster mushroom', 'cm', () => import('./oyster.json')),
  mushroom('honey-fungus', 'Honey fungus', 'cm', () => import('./honey-fungus.json')),
  mushroom('sulphur-tuft', 'Sulphur tuft', 'cm', () => import('./sulphur-tuft.json')),
  mushroom(
    'false-chanterelle',
    'False chanterelle',
    'cm',
    () => import('./false-chanterelle.json')
  ),
  mushroom('bitter-bolete', 'Bitter bolete', 'cm', () => import('./bitter-bolete.json'))
];

export function loadTemplate(id: string): Promise<Graph> {
  const template = templates.find((t) => t.id === id);
  if (!template) return Promise.reject(new Error(`Unknown template "${id}"`));
  return template.load();
}
