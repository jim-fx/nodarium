export type AvatarAnchor =
  | 'top-left'
  | 'top-right'
  | 'bottom-left'
  | 'bottom-right'
  | 'center'
  | 'right';

export type AvatarPosition = { x: number; y: number } | AvatarAnchor;

export interface HighlightTarget {
  /** CSS selector for the element to highlight */
  selector?: string;
  /** Name of an app-registered hook that returns Element | null */
  hookName?: string;
  /** Extra space around the element in px */
  padding?: number;
}

export interface DialogNode {
  text?: string;
  position?: AvatarPosition;
  highlight?: HighlightTarget;
  /** App hook to call on entering this node */
  action?: string;
  next?: string | null;
  choices?: Choice[];
  /** Called (and awaited) just before the avatar starts moving to this node */
  before?: StepCallback;
  /** Called (and awaited) just before the user leaves this node */
  after?: StepCallback;
}

export interface Choice {
  label: string;
  next?: string | null;
  onclick?: () => void;
}

export interface PlantyConfig {
  id: string;
  avatar?: {
    name?: string;
    defaultPosition?: AvatarPosition;
  };
  start: string;
  nodes: Record<string, DialogNode>;
}

export type PlantyHook = (
  ...args: unknown[]
) => void | Element | null | Promise<void> | (() => void);

/** Called before/after a node becomes active. Async-safe. */
export type StepCallback = (nodeId: string, node: DialogNode) => void | Promise<void>;
