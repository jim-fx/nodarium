<script lang="ts">
  import { DialogRunner } from '../dialog-runner.js';
  import type { AvatarPosition, DialogNode, PlantyConfig, PlantyHook } from '../types.js';
  import Highlight from './Highlight.svelte';
  import PlantyAvatar from './PlantyAvatar.svelte';
  import type { Mood } from './PlantyAvatar.svelte';
  import SpeechBubble from './SpeechBubble.svelte';

  interface Props {
    config: PlantyConfig;
    hooks?: Record<string, PlantyHook>;
    actions?: Record<string, PlantyHook>;
    onStepChange?: (nodeId: string, node: DialogNode) => void;
    onComplete?: () => void;
  }

  let { config, actions = {}, hooks = {}, onStepChange, onComplete }: Props = $props();

  const AVATAR_SIZE = 80;
  const SCREEN_PADDING = 20;

  // ── State ────────────────────────────────────────────────────────────
  let isActive = $state(false);
  let currentNodeId = $state<string | null>(null);
  let bubbleVisible = $state(false);
  let avatar = $state<PlantyAvatar>(null!);
  let avatarX = $state(0);
  let avatarY = $state(0);
  let mood = $state<Mood>('idle');
  let autoAdvanceTimer: ReturnType<typeof setTimeout> | null = null;
  let actionCleanup: (() => void) | null = null;

  // ── Derived ──────────────────────────────────────────────────────────
  const runner = $derived(new DialogRunner(config));
  const nextNode = $derived(
    runner.getNextNode(currentNodeId ?? '')
  );
  const mainPath = $derived(runner.getMainPath());
  const currentNode = $derived<DialogNode | null>(
    currentNodeId ? runner.getNode(currentNodeId) : null
  );
  const showBubble = $derived(
    isActive && bubbleVisible && currentNode !== null && !!currentNode.text
  );
  const highlight = $derived(currentNode?.highlight ?? null);
  const stepIndex = $derived(currentNodeId ? mainPath.indexOf(currentNodeId) : -1);
  const totalSteps = $derived(mainPath.length);

  // ── Position helpers ─────────────────────────────────────────────────
  function anchorToCoords(anchor: string): { x: number; y: number } {
    const w = window.innerWidth;
    const h = window.innerHeight;
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

  function resolvePosition(pos: AvatarPosition): { x: number; y: number } {
    return typeof pos === 'string' ? anchorToCoords(pos) : pos;
  }

  // ── Public API (exposed via bind:this) ───────────────────────────────
  export function start() {
    const defaultPos = config.avatar?.defaultPosition ?? 'bottom-right';
    const pos = resolvePosition(defaultPos);
    avatarX = pos.x;
    avatarY = pos.y;
    isActive = true;

    const start = runner.getStartNode();
    if (start) _enterNode(start.id, start.node);
  }

  export function stop() {
    _clearAutoAdvance();
    isActive = false;
    bubbleVisible = false;
    currentNodeId = null;
    mood = 'idle';
    onComplete?.();
  }

  export async function next() {
    if (!currentNodeId) return;
    await _runAfter(currentNodeId, currentNode);
    const next = runner.getNextNode(currentNodeId);
    if (next) _enterNode(next.id, next.node);
    else stop();
  }

  export function registerHook(name: string, fn: PlantyHook) {
    hooks = { ...hooks, [name]: fn };
  }

  // ── Internal ─────────────────────────────────────────────────────────

  async function _runAfter(nodeId: string, node: DialogNode | null) {
    if (!node) return;
    if (actionCleanup) {
      actionCleanup();
      actionCleanup = null;
    }
    await node.after?.(nodeId, node);
    await hooks[`after:${nodeId}`]?.(nodeId, node);
  }

  async function _enterNode(id: string, node: DialogNode) {
    _clearAutoAdvance();
    bubbleVisible = false;
    currentNodeId = id;
    onStepChange?.(id, node);

    // Before hooks — run before movement starts
    await node.before?.(id, node);
    await hooks[`before:${id}`]?.(id, node);

    // Fly to position first, then talk
    if (node.position) {
      mood = 'moving';
      const pos = resolvePosition(node.position);
      const hasChanges = pos.x !== avatarX || pos.y !== avatarY;
      avatarX = pos.x;
      avatarY = pos.y;
      if (hasChanges) await _wait(900);
    }

    mood = 'talking';
    bubbleVisible = true;

    // App hook
    if (node.action && actions[node.action]) {
      const result = await actions[node.action]();
      if (typeof result === 'function') actionCleanup = result as () => void;
    }

    const actionHook = hooks[`action:${id}`];
    if (actionHook) {
      const advance = () => {
        avatar.flash('happy', 2000);
        next();
      };
      const result = await actionHook(advance);
      if (typeof result === 'function') actionCleanup = result as () => void;
    }

    if (!node.choices && !node.next) {
      setTimeout(() => stop(), 3000);
    }

    // Stay in talking mood until the typewriter finishes (26 ms/char + buffer)
    const talkMs = (node.text?.length ?? 0) * 26 + 200;
    setTimeout(() => {
      mood = 'idle';
    }, talkMs);
  }

  function _wait(ms: number) {
    return new Promise<void>((r) => setTimeout(r, ms));
  }

  function _clearAutoAdvance() {
    if (autoAdvanceTimer !== null) {
      clearTimeout(autoAdvanceTimer);
      autoAdvanceTimer = null;
    }
  }
</script>

{#if isActive}
  <div class="pointer-events-none fixed inset-0 z-99999">
    <span>{currentNodeId}</span>
    {#if highlight}
      <Highlight selector={highlight.selector} hookName={highlight.hookName} {hooks} />
    {/if}

    <PlantyAvatar bind:this={avatar} bind:x={avatarX} bind:y={avatarY} {mood} />

    {#if showBubble && currentNode}
      <SpeechBubble
        text={currentNode.text ?? ''}
        {avatarX}
        {avatarY}
        choices={currentNode.choices || []}
        showNext={nextNode !== null}
        {stepIndex}
        {totalSteps}
        onNext={next}
        onClose={stop}
        onChoose={async (choice) => {
          await _runAfter(currentNodeId!, currentNode);
          if (choice && choice.action) {
            if (choice.action in actions) {
              actions[choice.action]();
            } else {
              console.warn(`Planty: No action found for ${choice.action}`);
            }
            return;
          }
          if (!choice.next) {
            stop();
            return;
          }
          const n = runner.followChoice(choice);
          if (n) _enterNode(n.id, n.node);
          else stop();
        }}
      />
    {/if}
  </div>
{/if}
