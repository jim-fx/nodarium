import type { FrameColor, NodeDefinition, NodeInput } from '@nodarium/runtime';
import type { Component } from 'svelte';
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
  /** drawn in the help next to the description of the hovered node, renders nothing for a node it has no picture of */
  helpSketch?: Component<{ node: NodeDefinition }>;
  /** node ids and timings on the nodes */
  showDebugInfo: boolean;
  /** the name of the theme, the colours are read again when it changes */
  theme: string;
  colors: ThemeColors;
  /** CSS colour of the bar on the node header per collection, `max/plantarium`; a collection without one gets no bar */
  systemColors?: Record<string, string>;
  /**
   * CSS colour of the links and sockets per link type, as the links look, `{ fx: '#c65a19' }`; also
   * replaces the colour of a built-in type. A type without one gets a colour from its name.
   */
  linkColors?: Record<string, string>;
  safePadding?: { left?: number; right?: number; bottom?: number; top?: number };
  nodeTimings: Record<number, number>;
  /**
   * Only the values of the inputs can be changed: nodes stay where they are, nothing is added,
   * removed or connected. Read once, when the editor is created.
   */
  valuesOnly?: boolean;
  /** Node that made the last execution fail */
  errorNodeId?: number;
};

/** A short text drawn next to the label of an input, like the signal the app maps onto it */
export type InputBadge = (nodeId: number, key: string) => string | undefined;

export type InputContextMenuEvent = {
  nodeId: number;
  /** the type of the node, `max/core/float` */
  nodeType: string;
  /** the key of the input */
  key: string;
  /** its definition, with the exposed state of the node */
  input: NodeInput;
  /** the pointer, in client coordinates */
  x: number;
  y: number;
  /** the editor opens no add menu and the browser no menu of its own */
  preventDefault(): void;
};
