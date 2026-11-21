import { CORE_SKETCHES } from './core';
import type { NodeSketch } from './kit';
import { MYZEL_SKETCHES } from './myzel';
import { PLANTARIUM_SKETCHES } from './plantarium';

export {
  type NodeSketch,
  PANEL,
  SKETCH_PERIOD,
  type SketchFrame,
  sketchPhase,
  type SketchPoint,
  type SketchShape,
  type SketchTone
} from './kit';

export const NODE_SKETCHES: Record<string, NodeSketch> = {
  ...CORE_SKETCHES,
  ...PLANTARIUM_SKETCHES,
  ...MYZEL_SKETCHES
};

/** The nodes that get no sketch, and why: their name and inputs already say what they do */
export const UNSKETCHED: Record<string, string> = {
  'max/core/float': 'a number set on the node',
  'max/core/output': 'the end of every graph, it only shows what reaches it',
  'max/plantarium/stem': 'makes straight stems, the name says it',
  'max/plantarium/shape': 'the node shows the outline drawn on it',
  'max/plantarium/join': 'puts lists together, nothing changes shape',
  'max/plantarium/set_color': 'colours the paths, the colour is on the node',
  'max/myzel/mesh': 'picks a scanned substrate, the options name it'
};

export function sketchFor(nodeType: string): NodeSketch | undefined {
  return NODE_SKETCHES[nodeType];
}
