/** Size of the bed the plants grow in, x is width, z is depth */
export const BED_WIDTH = 16;
export const BED_DEPTH = 9;
/** Distance between plants and the border of the bed */
export const BED_MARGIN = 1.1;

/** Only the most recently modified projects are planted */
export const MAX_PLANTS = 12;
/** Every plant is scaled to fit into this cylinder */
export const PLANT_MAX_HEIGHT = 3.2;
/** branch tips may reach over the edge of their island */
export const PLANT_MAX_RADIUS = 2.2;
/** Radius of the patch of ground a plant claims for itself */
export const PLANT_ISLAND_RADIUS = 1.1;
/** Preferred distance between two plants */
export const PLANT_SPACING = 3;

export type GardenStyle = 'zen' | 'cottage';
export const GARDEN_STYLES: GardenStyle[] = ['zen', 'cottage'];

/** What the shared parts of the garden need to know about a style */
export interface StyleSettings {
  /** color of the plants grown from the projects */
  plant: string;
  /** the moss cushion at the root of every plant */
  cushion: boolean;
  /** the fog starts this far behind the camera target and is this long */
  fog: { start: number; length: number };
  /** aperture of the depth of field, for the overview and a focused plant */
  aperture: { overview: number; focus: number };
  /** strongest blur, far out of focus */
  maxBlur: number;
  bloom: number;
  vignette: number;
}
