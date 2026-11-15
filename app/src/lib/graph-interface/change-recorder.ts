import type { Graph } from '@nodarium/types';
import { HistoryManager } from './history-manager';

/** The undo history of a graph, and its save once the changes rest */
export class ChangeRecorder {
  readonly history = new HistoryManager();

  // A value that changes on every tick of a drag becomes one undo step once it rests
  private stepWhenIdle = false;
  private idleTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private options: {
      /** the graph as it is now */
      snapshot: () => Graph;
      onsave: () => void;
      idleMs?: number;
    }
  ) {}

  /** An undo step now, the save once the changes rest */
  step() {
    this.stepWhenIdle = false;
    this.history.save(this.options.snapshot());
    this.saveWhenIdle();
  }

  /**
   * The undo step and the save once the changes rest: a value that changes on every tick of a
   * drag becomes one step, holding the value from before the drag
   */
  coalesce() {
    this.stepWhenIdle = true;
    this.saveWhenIdle();
  }

  /** The pending undo step and the save right away, e.g. before the page is closed */
  flush() {
    if (this.stepWhenIdle) {
      this.stepWhenIdle = false;
      this.history.save(this.options.snapshot());
    }
    if (this.idleTimer === undefined) return;
    clearTimeout(this.idleTimer);
    this.idleTimer = undefined;
    this.options.onsave();
  }

  undo(): Graph | undefined {
    this.flush();
    return this.history.undo();
  }

  redo(): Graph | undefined {
    this.flush();
    return this.history.redo();
  }

  reset() {
    this.history.reset();
    this.stepWhenIdle = false;
  }

  private saveWhenIdle() {
    clearTimeout(this.idleTimer);
    this.idleTimer = setTimeout(() => this.flush(), this.options.idleMs ?? 300);
  }
}
