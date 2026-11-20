import type { FrameColor } from '@nodarium/types';
import type { Color } from 'three';

/** The colours the editor draws with. The objects stay, a new theme changes their values in place. */
export type ThemeColors = Record<
  | 'layer-0'
  | 'layer-1'
  | 'layer-2'
  | 'outline'
  | 'active'
  | 'selected'
  | 'error'
  | `frame-${FrameColor}`,
  Color
>;

export type GraphOptions = {
  backgroundType: 'grid' | 'dots' | 'none';
  snapToGrid: boolean;
  showHelp: boolean;
  /** node ids and timings on the nodes */
  showDebugInfo: boolean;
  /** the name of the theme, the colours are read again when it changes */
  theme: string;
  colors: ThemeColors;
  /** CSS colour of the bar on the node header per collection, `max/plantarium`; a collection without one gets no bar */
  systemColors?: Record<string, string>;
  safePadding?: { left?: number; right?: number; bottom?: number; top?: number };
  nodeTimings: Record<number, number>;
  /** Node that made the last execution fail */
  errorNodeId?: number;
};
