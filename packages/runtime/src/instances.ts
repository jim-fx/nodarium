import type { NodeDefinition } from '@nodarium/types';

/**
 * A wasm memory never shrinks, it keeps the size of the biggest run its node ever had, and a
 * process only has room for about 125 of them. So after every run the executor releases the
 * instances of the nodes that are not in the graph any more, and those with more memory than
 * this. A released node instantiates its compiled module again the next time it runs.
 */
export const RELEASE_INSTANCES_OVER = 32 * 2 ** 20;

/** Which node instances may hold memory, and when they are let go */
export class InstanceTracker {
  /** Bytes of memory a node may keep after a run, see {@link RELEASE_INSTANCES_OVER} */
  releaseOver = RELEASE_INSTANCES_OVER;

  // every definition that may hold results in its memory, they stay there until it is reset
  private used = new Map<string, NodeDefinition>();
  // runs can overlap while they load their nodes, only the last one to finish releases
  private activeRuns = 0;
  // the types of every run since the last release
  private typesInUse = new Set<string>();
  private releaseAllWhenIdle = false;

  enterRun() {
    this.activeRuns++;
  }

  /**
   * Remembers the definitions of this run and frees the results of the previous one. Nothing
   * of the run may await after this.
   */
  resetFor(definitions: Map<string, NodeDefinition>) {
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
      all
      || !inUse.has(type)
      || (definition.memoryBytes?.() ?? 0) > this.releaseOver
    );
  }

  /** All of them, now or once no run is going */
  releaseAll() {
    if (this.activeRuns > 0) this.releaseAllWhenIdle = true;
    else this.release();
  }

  private release(filter?: (type: string, definition: NodeDefinition) => boolean) {
    for (const [type, definition] of this.used) {
      if (!definition.release || (filter && !filter(type, definition))) continue;
      definition.release();
      this.used.delete(type);
    }
  }
}
