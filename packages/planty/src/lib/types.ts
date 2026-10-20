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
  /** App action to call on entering this node */
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
  /** Runs an app action instead of navigating, the dialog stays on the current node */
  action?: string;
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

/** Returned by actions and step hooks, called when the user leaves the node */
export type Cleanup = () => void;

type MaybePromise<T> = T | Promise<T>;

/** Runs when a node with a matching `action` (or a choice with one) is entered */
export type PlantyAction = () => MaybePromise<void | Cleanup>;

/**
 * Registered as `action:<nodeId>`, waits for the user to do something.
 * Call `advance` to move on to the next node.
 */
export type StepActionHook = (advance: () => void) => MaybePromise<void | Cleanup>;

/** Registered under a highlight's `hookName`, resolves the element to highlight */
export type HighlightHook = () => Element | null;

/** Called before/after a node becomes active. Async-safe. */
export type StepCallback = (nodeId: string, node: DialogNode) => MaybePromise<void>;

export type PlantyHook = StepActionHook | StepCallback | HighlightHook;

export interface PlantyHooks {
  [key: `action:${string}`]: StepActionHook;
  [key: `before:${string}`]: StepCallback;
  [key: `after:${string}`]: StepCallback;
  [key: string]: PlantyHook;
}
