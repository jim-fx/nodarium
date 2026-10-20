<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { prefersReducedMotion } from 'svelte/motion';
  import { innerHeight, innerWidth } from 'svelte/reactivity/window';
  import { DialogRunner } from '../dialog-runner.js';
  import type {
    AvatarAnchor,
    AvatarPosition,
    Choice,
    Cleanup,
    DialogNode,
    PlantyAction,
    PlantyConfig,
    PlantyHook,
    PlantyHooks
  } from '../types.js';
  import Highlight from './Highlight.svelte';
  import PlantyAvatar from './PlantyAvatar.svelte';
  import type { Mood } from './PlantyAvatar.svelte';
  import SpeechBubble from './SpeechBubble.svelte';

  interface Props {
    config: PlantyConfig;
    hooks?: PlantyHooks;
    actions?: Record<string, PlantyAction>;
    /** Start the tour on mount unless this config has been started before */
    autoStart?: boolean;
    onStepChange?: (nodeId: string, node: DialogNode) => void;
    onComplete?: () => void;
  }

  let { config, actions = {}, hooks = {}, autoStart = false, onStepChange, onComplete }: Props =
    $props();

  const AVATAR_SIZE = 80;
  const SCREEN_PADDING = 20;
  const FLY_MS = 900;
  const AUTO_STOP_MS = 3000;

  // ── State ────────────────────────────────────────────────────────────
  let isActive = $state(false);
  let currentNodeId = $state<string | null>(null);
  let bubbleVisible = $state(false);
  let avatar = $state<PlantyAvatar>();
  let avatarX = $state(0);
  let avatarY = $state(0);
  let mood = $state<Mood>('idle');
  /** Anchor the avatar sits at, null once the user dragged it somewhere else */
  let anchor = $state<AvatarAnchor | null>(null);
  /** Nodes visited since the tour started or the user picked a choice */
  let segment = $state<string[]>([]);

  let cleanups: Cleanup[] = [];
  // eslint-disable-next-line svelte/prefer-svelte-reactivity -- not rendered
  const timers = new Set<ReturnType<typeof setTimeout>>();
  /** Bumped whenever the active node is left, async work of older nodes checks it and bails */
  let generation = 0;

  // ── Derived ──────────────────────────────────────────────────────────
  const runner = $derived(new DialogRunner(config));
  const nextNode = $derived(runner.getNextNode(currentNodeId ?? ''));
  const currentNode = $derived<DialogNode | null>(
    currentNodeId ? runner.getNode(currentNodeId) : null
  );
  const showBubble = $derived(
    isActive && bubbleVisible && currentNode !== null && !!currentNode.text
  );
  const highlight = $derived(currentNode?.highlight ?? null);
  const remainingSteps = $derived(
    currentNodeId ? runner.getChain(currentNodeId).length - 1 : 0
  );
  const stepIndex = $derived(segment.length - 1);
  const totalSteps = $derived(segment.length + remainingSteps);
  const seenKey = $derived(`planty:${config.id}:seen`);

  $effect(() => {
    const problems = runner.validate();
    if (problems.length) {
      console.warn(`Planty: problems in config "${config.id}":\n${problems.join('\n')}`);
    }
  });

  // Keep the avatar on screen when the window is resized
  $effect(() => {
    const w = innerWidth.current ?? 0;
    const h = innerHeight.current ?? 0;
    untrack(() => {
      if (!isActive) return;
      if (anchor) {
        const pos = anchorToCoords(anchor);
        avatarX = pos.x;
        avatarY = pos.y;
      } else {
        avatarX = Math.max(0, Math.min(avatarX, w - AVATAR_SIZE));
        avatarY = Math.max(0, Math.min(avatarY, h - AVATAR_SIZE));
      }
    });
  });

  onMount(() => {
    if (autoStart && !_hasSeen()) start();
  });

  // ── Position helpers ─────────────────────────────────────────────────
  function anchorToCoords(anchor: AvatarAnchor): { x: number; y: number } {
    const w = innerWidth.current ?? 0;
    const h = innerHeight.current ?? 0;
    switch (anchor) {
      case 'top-left':
        return { x: SCREEN_PADDING, y: SCREEN_PADDING };
      case 'top-right':
        return { x: w - AVATAR_SIZE - SCREEN_PADDING, y: SCREEN_PADDING };
      case 'bottom-left':
        return { x: SCREEN_PADDING, y: h - AVATAR_SIZE - SCREEN_PADDING };
      case 'center':
        return { x: (w - AVATAR_SIZE) / 2, y: (h - AVATAR_SIZE) / 2 };
      case 'right':
        return { x: w - AVATAR_SIZE - SCREEN_PADDING, y: (h - AVATAR_SIZE) / 2 };
      case 'bottom-right':
      default:
        return { x: w - AVATAR_SIZE - SCREEN_PADDING, y: h - AVATAR_SIZE - SCREEN_PADDING };
    }
  }

  function moveTo(pos: AvatarPosition) {
    anchor = typeof pos === 'string' ? pos : null;
    const coords = typeof pos === 'string' ? anchorToCoords(pos) : pos;
    const moved = coords.x !== avatarX || coords.y !== avatarY;
    avatarX = coords.x;
    avatarY = coords.y;
    return moved;
  }

  // ── Public API (exposed via bind:this) ───────────────────────────────
  export function start() {
    if (isActive) _reset();
    _markSeen();
    moveTo(config.avatar?.defaultPosition ?? 'bottom-right');
    isActive = true;

    const start = runner.getStartNode();
    if (start) _enterNode(start.id, start.node, true);
  }

  export function stop() {
    if (!isActive) return;
    _reset();
    onComplete?.();
  }

  export async function next() {
    if (!currentNodeId) return;
    const id = currentNodeId;
    const gen = ++generation;
    await _runAfter(id, currentNode);
    if (gen !== generation || !isActive) return;
    const next = runner.getNextNode(id);
    if (next) _enterNode(next.id, next.node);
    else stop();
  }

  export function registerHook(name: string, fn: PlantyHook) {
    hooks = { ...hooks, [name]: fn } as PlantyHooks;
  }

  // ── Internal ─────────────────────────────────────────────────────────

  function _reset() {
    generation++;
    _runCleanups();
    _clearTimers();
    isActive = false;
    bubbleVisible = false;
    currentNodeId = null;
    segment = [];
    mood = 'idle';
  }

  async function _choose(choice: Choice) {
    if (choice.action) {
      const gen = generation;
      _runAction(choice.action, () => gen === generation && isActive);
      return;
    }
    if (!choice.next) {
      stop();
      return;
    }
    if (!currentNodeId) return;
    const gen = ++generation;
    await _runAfter(currentNodeId, currentNode);
    if (gen !== generation || !isActive) return;
    const n = runner.followChoice(choice);
    if (n) _enterNode(n.id, n.node, true);
    else stop();
  }

  async function _runAfter(nodeId: string, node: DialogNode | null) {
    _runCleanups();
    _clearTimers();
    if (!node) return;
    await node.after?.(nodeId, node);
    await hooks[`after:${nodeId}`]?.(nodeId, node);
  }

  async function _enterNode(id: string, node: DialogNode, resetSegment = false) {
    const gen = ++generation;
    const alive = () => gen === generation && isActive;

    _clearTimers();
    bubbleVisible = false;
    currentNodeId = id;
    segment = resetSegment ? [id] : [...segment, id];
    onStepChange?.(id, node);

    // Before hooks — run before movement starts
    await node.before?.(id, node);
    await hooks[`before:${id}`]?.(id, node);
    if (!alive()) return;

    // Fly to position first, then talk
    if (node.position) {
      mood = 'moving';
      if (moveTo(node.position) && !prefersReducedMotion.current) await _wait(FLY_MS);
      if (!alive()) return;
    }

    mood = 'talking';
    bubbleVisible = true;

    if (node.action) await _runAction(node.action, alive);
    if (!alive()) return;

    const actionHook = hooks[`action:${id}`];
    if (actionHook) {
      const advance = () => {
        if (!alive()) return;
        avatar?.flash('happy', 2000);
        next();
      };
      _track(await actionHook(advance), alive);
      if (!alive()) return;
    }

    if (!node.choices && !node.next) {
      _setTimer(() => stop(), AUTO_STOP_MS);
    }
  }

  async function _runAction(name: string, alive: () => boolean) {
    const action = actions[name];
    if (!action) {
      console.warn(`Planty: No action found for "${name}"`);
      return;
    }
    _track(await action(), alive);
  }

  /** Keep a cleanup until the node is left, or run it right away if that already happened */
  function _track(result: void | Cleanup, alive: () => boolean) {
    if (typeof result !== 'function') return;
    if (alive()) cleanups.push(result);
    else result();
  }

  function _runCleanups() {
    const pending = cleanups;
    cleanups = [];
    for (const cleanup of pending) cleanup();
  }

  function _setTimer(fn: () => void, ms: number) {
    const id = setTimeout(() => {
      timers.delete(id);
      fn();
    }, ms);
    timers.add(id);
  }

  function _clearTimers() {
    for (const id of timers) clearTimeout(id);
    timers.clear();
  }

  function _wait(ms: number) {
    return new Promise<void>((r) => _setTimer(r, ms));
  }

  function _hasSeen() {
    try {
      return localStorage.getItem(seenKey) !== null;
    } catch {
      return false;
    }
  }

  function _markSeen() {
    try {
      localStorage.setItem(seenKey, new Date().toISOString());
    } catch {
      // storage unavailable, the tour just auto-starts again next time
    }
  }
</script>

{#if isActive}
  <div class="pointer-events-none fixed inset-0 z-99999">
    {#if highlight}
      <Highlight
        selector={highlight.selector}
        hookName={highlight.hookName}
        padding={highlight.padding}
        {hooks}
      />
    {/if}

    <PlantyAvatar
      bind:this={avatar}
      bind:x={avatarX}
      bind:y={avatarY}
      {mood}
      ondrag={() => (anchor = null)}
    />

    {#if showBubble && currentNode}
      <SpeechBubble
        title={config.avatar?.name ?? 'Planty'}
        text={currentNode.text ?? ''}
        {avatarX}
        {avatarY}
        choices={currentNode.choices || []}
        showNext={nextNode !== null}
        {stepIndex}
        {totalSteps}
        onNext={next}
        onClose={stop}
        onChoose={_choose}
        onTypingDone={() => {
          if (mood === 'talking') mood = 'idle';
        }}
      />
    {/if}
  </div>
{/if}
