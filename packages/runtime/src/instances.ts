import type { NodeDefinition } from '@nodarium/types';

/**
 * Bytes of memory above which an instance is released after every run, also when its node
 * stays in the graph. A released node instantiates its compiled module again the next time it
 * runs, and its memory has to grow from zero again, so by default no instance is released for
 * its size.
 */
export const RELEASE_INSTANCES_OVER = Infinity;

/**
 * Instances an executor keeps after a run. A process only has room for about 125 wasm memories,
 * shared by the runtime, the thumbnail and the garden worker.
 */
export const KEEP_INSTANCES = 40;

function memoryOf(definition: NodeDefinition) {
  return definition.memoryBytes?.() ?? 0;
}

function holdsInstance(definition: NodeDefinition) {
  return definition.memoryBytes ? definition.memoryBytes() > 0 : true;
}

/**
 * Which node instances may hold memory, and when they are let go. A wasm memory never shrinks,
 * so after a run the instances of the nodes that left the graph are released, and the biggest
 * ones once there are more than {@link KEEP_INSTANCES}.
 */
export class InstanceTracker {
  /** see {@link RELEASE_INSTANCES_OVER} */
  releaseOver = RELEASE_INSTANCES_OVER;
  /** see {@link KEEP_INSTANCES} */
  keep = KEEP_INSTANCES;

  // every definition that may hold results in its memory, they stay there until it is reset
  private used = new Map<string, NodeDefinition>();
  // runs can overlap while they load their nodes, only the last one to finish releases
  private activeRuns = 0;
  // the types of every run since the last release
  private typesInUse = new Set<string>();
  // the types of the run that started last, its graph is the one the user sees
  private latestTypes = new Set<string>();
  private runsStarted = 0;
  private releaseAllWhenIdle = false;

  /** Returns the number of the run, which it passes to {@link resetFor} */
  enterRun() {
    this.activeRuns++;
    return ++this.runsStarted;
  }

  /**
   * Remembers the definitions of this run and frees the results of the previous one. Nothing
   * of the run may await after this.
   */
  resetFor(definitions: Map<string, NodeDefinition>, run: number) {
    if (run === this.runsStarted) this.latestTypes = new Set(definitions.keys());
    for (const [type, definition] of definitions) {
      this.used.set(type, definition);
      this.typesInUse.add(type);
    }
    for (const definition of this.used.values()) {
      definition.reset?.();
    }
  }

  /**
   * The last run to leave releases what the graph doesn't need. The results, the cache entries
   * and the debug data are copies, nothing points into a memory.
   */
  leaveRun() {
    this.activeRuns--;
    if (this.activeRuns !== 0) return;
    const all = this.releaseAllWhenIdle;
    const inUse = this.typesInUse;
    this.releaseAllWhenIdle = false;
    this.typesInUse = new Set();
    this.release((type, definition) =>
      all || !inUse.has(type) || memoryOf(definition) > this.releaseOver
    );
    this.releaseOverLimit();
  }

  /** All of them, now or once no run is going */
  releaseAll() {
    if (this.activeRuns > 0) this.releaseAllWhenIdle = true;
    else this.release();
  }

  // the biggest of the nodes outside the latest graph first, then the biggest of it
  private releaseOverLimit() {
    const live = [...this.used].filter(([, definition]) =>
      definition.release && holdsInstance(definition)
    );
    const excess = live.length - this.keep;
    if (excess <= 0) return;
    const byRank = live
      .map(([type, definition]) => ({
        type,
        current: this.latestTypes.has(type),
        bytes: memoryOf(definition)
      }))
      .sort((a, b) => Number(a.current) - Number(b.current) || b.bytes - a.bytes);
    const victims = new Set(byRank.slice(0, excess).map(({ type }) => type));
    this.release((type) => victims.has(type));
  }

  private release(filter?: (type: string, definition: NodeDefinition) => boolean) {
    for (const [type, definition] of this.used) {
      if (!definition.release || (filter && !filter(type, definition))) continue;
      definition.release();
      this.used.delete(type);
    }
  }
}
