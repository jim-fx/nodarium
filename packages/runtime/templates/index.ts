import type { Graph, ProjectUnit } from '@nodarium/types';
import { substrates, withSubstrate } from './substrate';

export {
  type Substrate,
  type SubstrateKind,
  substrateKinds,
  substrates,
  withSubstrate
} from './substrate';

export type TemplateCategory = 'Plants' | 'Mushrooms';

export type Template = {
  /** The file name of the template, it is imported on its own from `@nodarium/runtime/templates/<id>` */
  id: string;
  name: string;
  category: TemplateCategory;
  /** The unit the app shows lengths in, the graph stores meters either way */
  unit: ProjectUnit;
  /** The species a plant template models, as `docs/PLANT_TEMPLATES.md` names it */
  species?: string;
  /** One line on how the graph builds it */
  notes?: string;
  /** The topic of `nodarium docs` that describes the template in full */
  doc?: 'plant-templates' | 'myzel';
  /** The graph is shared by everyone who loads it, clone it before changing it */
  load: () => Promise<Graph>;
};

export const templateCategories: TemplateCategory[] = ['Plants', 'Mushrooms'];

type About = { species?: string; notes: string };

// from the template sections of docs/PLANT_TEMPLATES.md and the table of docs/MYZEL.md
const ABOUT: Record<string, About> = {
  fern: {
    species: 'Nephrolepis exaltata (Boston fern)',
    notes: '22 fronds from a spiral at the golden angle, arched by gravity, each with 110 '
      + 'pinnae in two ranks.'
  },
  agave: {
    species: 'Agave americana (century plant)',
    notes: 'One channelled leaf shape on two spirals: 13–22 outer leaves low and 19–31 inner '
      + 'leaves rising into the cone, each turned about the golden angle from the last.'
  },
  grass: {
    species: 'Cenchrus purpurascens (fountain grass)',
    notes: '22 arching culms, a plume of 140 bristles at the top of each and eight leaf blades at '
      + 'its base.'
  },
  palm: {
    species: 'Cocos nucifera (coconut palm)',
    notes: 'A leaning trunk with 26 fronds at the golden angle in its top, each with 120 narrow, '
      + 'folded leaflets.'
  },
  sapling: {
    species: 'Betula pendula (silver birch), young tree',
    notes:
      'A straight trunk with 16 branches at the golden angle, each with 12 hanging twigs, birch leaves '
      + 'from a node group.'
  },
  lavender: {
    species: 'Lavandula angustifolia (English lavender)',
    notes: '11–18 woody stems from a spiral, each with up to 4 upright flower stems of '
      + 'linear leaves and a spike of 110 florets, and a short leafy side shoot.'
  },
  sage: {
    species: 'Salvia officinalis (common sage)',
    notes: '9 woody stems from a spiral, each with 7 shoots of opposite, decussate leaf '
      + 'pairs.'
  },
  bamboo: {
    species: 'Phyllostachys aurea (golden bamboo)',
    notes: '11 culms with 28 branches in pairs from half-way up, 2–5 leaves near each branch tip.'
  },
  'lady-fern': {
    species: 'Athyrium filix-femina (lady fern)',
    notes: '14 fronds arched into a vase, each with 40–50 pinnae in two ranks.'
  },
  maidenhair: {
    species: 'Adiantum raddianum (delta maidenhair)',
    notes: '10–25 wiry fronds arched into a mound, a deltate, 3-pinnate blade on the upper half of '
      + 'each.'
  },
  daisy: {
    species: 'Bellis perennis (common daisy)',
    notes: '3–8 scapes over a flat rosette of 12–26 leaves on a short crown, a head of disc, rays '
      + 'and bracts on each tip.'
  },
  horsetail: {
    species: 'Equisetum arvense (field horsetail)',
    notes: 'Five stems, each with 14 nodes of sheath teeth and a whorl of 9–12 branches at each.'
  },
  'devils-backbone': {
    species: 'Euphorbia tithymaloides (zigzag plant)',
    notes: '5–10 stems zigzagged by trunk_kink that come up one after another, an ovate leaf on '
      + 'the outer side of every kink, red cyathia on the tips of the oldest. A low bush or a tall '
      + 'open plant by seed.'
  },
  knotweed: {
    species: 'Reynoutria japonica (Japanese knotweed)',
    notes: '4–10 arching, jointed canes that zigzag, a swollen node every 11 cm, a large leaf and '
      + 'a branch of hanging leaves at each of the upper 13 nodes.'
  },
  'wild-carrot': {
    species: "Daucus carota (Queen Anne's lace)",
    notes: 'A main stem with 3 side branches and 3 stalks from the base, an open umbel or a bud on '
      + 'every tip.'
  },
  dandelion: {
    species: 'Taraxacum officinale (common dandelion)',
    notes: 'A taproot, a rosette of 8–18 runcinate leaf_surface leaves on a spiral, 3–10 scapes '
      + 'that curve up, each with its own phase: a bud, an open head of 130–165 ligules, a closed '
      + 'head, a clock of 110 baked seeds that fly off. A habit per seed, from lawn to meadow.'
  },
  yarrow: {
    species: 'Achillea millefolium (common yarrow)',
    notes: '1–3 upright stems with feathery leaves of paths, flat-topped clusters of small white '
      + 'heads on an umbel at the tip of the stem and of up to five side branches.'
  },
  mullein: {
    species: 'Verbascum thapsus (great mullein)',
    notes: 'A mound of 26–42 woolly leaf_surface leaves in one rosette, a stem of 0.9–2.1 m with '
      + 'clasping leaves and decurrent wings that shrink to the spike, and a spike of 200–260 bud '
      + 'clusters with a band of open flowers.'
  },
  plantain: {
    species: 'Plantago lanceolata (ribwort plantain)',
    notes: 'A rosette of 9–24 narrow, striped leaves and 4–14 scapes, each with a spike '
      + 'that flowers from the base up, a ring of anthers between its brown and green part.'
  },
  'champignon-groomed': { notes: 'The Stipe, Cap, Gills and Veil groups, grown by time.' },
  'fly-agaric': { notes: 'As the champignon, with a Cuticle and a Volva group.' },
  porcini: {
    notes: 'The Stipe and Cap groups, a Pores group instead of the Gills and a Reticulation net on '
      + 'the stipe.'
  },
  shiitake: { notes: 'The Stipe, Cap, Gills and Cuticle groups, no veil.' },
  'shaggy-inkcap': {
    notes: 'Three to seven stipes of their own age, with the Gills, Cuticle and Ring groups.'
  },
  chanterelle: {
    notes: 'Four to nine stipes of their own age, ridges instead of gills, a Stipe base group.'
  },
  oyster: {
    notes: 'Four to eight stipes from one spot, each of its own age, their shell caps in tiers.'
  },
  'honey-fungus': {
    notes: 'Six to eleven stipes of their own age, with the Gills, Ring and Cuticle groups.'
  },
  'clouded-agaric': {
    species: 'Clitocybe nebularis',
    notes: 'Five to ten stipes on an arc of a ring, each of its own age, with the Gills and '
      + 'Cuticle groups.'
  },
  'sulphur-tuft': { notes: 'Eight to fourteen thin stipes, like the honey fungus.' },
  'false-chanterelle': { notes: 'Like the chanterelle, with lamellae instead of ridges.' },
  'bitter-bolete': {
    notes: 'Like the porcini, its pores white turning pink and a coarse dark net on the stipe.'
  }
};

// every template is imported on its own, so a bundler splits them into chunks of their own
function template(category: TemplateCategory) {
  type Load = () => Promise<{ default: unknown }>;
  return (id: string, name: string, unit: ProjectUnit, load: Load): Template => ({
    id,
    name,
    category,
    unit,
    ...(ABOUT[id] && { ...ABOUT[id], doc: category === 'Plants' ? 'plant-templates' : 'myzel' }),
    load: () => load().then((m) => m.default as Graph)
  });
}

const plant = template('Plants');
const mushroom = template('Mushrooms');

/** In the order of the project browser of the app */
export const templates: Template[] = [
  plant('default-plant', 'Default Plant', 'm', () => import('./default-plant.json')),
  plant('fern', 'Fern', 'cm', () => import('./fern.json')),
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
  plant('devils-backbone', "Devil's Backbone", 'cm', () => import('./devils-backbone.json')),
  plant('knotweed', 'Japanese Knotweed', 'm', () => import('./knotweed.json')),
  plant('wild-carrot', 'Wild Carrot', 'cm', () => import('./wild-carrot.json')),
  plant('dandelion', 'Dandelion', 'cm', () => import('./dandelion.json')),
  plant('yarrow', 'Yarrow', 'cm', () => import('./yarrow.json')),
  plant('mullein', 'Great Mullein', 'm', () => import('./mullein.json')),
  plant('plantain', 'Ribwort Plantain', 'cm', () => import('./plantain.json')),
  mushroom('champignon-groomed', 'Champignon', 'cm', () => import('./champignon-groomed.json')),
  mushroom('fly-agaric', 'Fly agaric', 'cm', () => import('./fly-agaric.json')),
  mushroom('porcini', 'Porcini', 'cm', () => import('./porcini.json')),
  mushroom('shiitake', 'Shiitake', 'cm', () => import('./shiitake.json')),
  mushroom('shaggy-inkcap', 'Shaggy inkcap', 'cm', () => import('./shaggy-inkcap.json')),
  mushroom('chanterelle', 'Chanterelle', 'cm', () => import('./chanterelle.json')),
  mushroom(
    'false-chanterelle',
    'False chanterelle',
    'cm',
    () => import('./false-chanterelle.json')
  ),
  mushroom('oyster', 'Oyster mushroom', 'cm', () => import('./oyster.json')),
  mushroom('honey-fungus', 'Honey fungus', 'cm', () => import('./honey-fungus.json')),
  mushroom('clouded-agaric', 'Clouded agaric', 'cm', () => import('./clouded-agaric.json'))
];

/**
 * Templates the project browser doesn't list any more. They stay in the package for the
 * projects that load them, `loadTemplate()` finds them by their id
 */
export const unlistedTemplates: Template[] = [
  mushroom('sulphur-tuft', 'Sulphur tuft', 'cm', () => import('./sulphur-tuft.json')),
  mushroom('bitter-bolete', 'Bitter bolete', 'cm', () => import('./bitter-bolete.json'))
];

export type LoadTemplateOptions = {
  /** Puts the mushrooms on the substrate of `substrates[id]`, see `withSubstrate()` */
  substrate?: boolean;
};

export async function loadTemplate(id: string, options: LoadTemplateOptions = {}): Promise<Graph> {
  const template = [...templates, ...unlistedTemplates].find((t) => t.id === id);
  if (!template) throw new Error(`Unknown template "${id}"`);
  const graph = await template.load();
  if (!options.substrate) return graph;
  const substrate = substrates[id];
  if (!substrate) throw new Error(`The template "${id}" has no substrate`);
  return withSubstrate(graph, substrate);
}
