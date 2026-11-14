import type { Graph } from '@nodarium/types';
import { createLogger } from '@nodarium/utils';
import { create, type Delta } from 'jsondiffpatch';
import { clone } from './helpers/index.ts';

const diff = create({
  objectHash: function(obj, index) {
    if (obj === null) return obj;
    if ('id' in obj) return obj.id as string;
    if ('_id' in obj) return obj._id as string;
    if (Array.isArray(obj)) {
      return obj.join('-');
    }
    return '$$index:' + index;
  }
});

const log = createLogger('history');
log.mute();

export class HistoryManager {
  index: number = -1;
  history: Delta[] = [];
  private initialState: Graph | undefined;
  private state: Graph | undefined;

  private opts = {
    debounce: 400,
    maxHistory: 100
  };

  constructor({ maxHistory = 100, debounce = 100 } = {}) {
    this.history = [];
    this.index = -1;
    this.opts.debounce = debounce;
    this.opts.maxHistory = maxHistory;
  }

  save(state: Graph) {
    if (!this.state) {
      this.state = clone(state);
      this.initialState = this.state;
      log.log('initial state saved');
    } else {
      const newState = state;
      const delta = diff.diff(this.state, newState);
      if (delta) {
        log.log('saving state');
        if (this.index < this.history.length - 1) {
          this.history.splice(this.index + 1);
        }

        this.history.push(delta);
        this.index++;

        if (this.history.length > this.opts.maxHistory) {
          const dropped = this.history.shift()!;
          this.index--;
          this.initialState = diff.patch(this.initialState, dropped) as Graph;
        }
        this.state = newState;
      } else {
        log.log('no changes');
      }
    }
  }

  reset() {
    this.history = [];
    this.index = -1;
    this.state = undefined;
    this.initialState = undefined;
  }

  undo() {
    if (this.index === -1 && this.initialState) {
      log.log('reached start, loading initial state');
      return clone(this.initialState);
    } else {
      const delta = this.history[this.index];
      const prevState = diff.unpatch(this.state, delta) as Graph;
      this.state = prevState;
      this.index = Math.max(-1, this.index - 1);
      return clone(prevState);
    }
  }

  redo() {
    if (this.index < this.history.length - 1) {
      const nextIndex = Math.min(this.history.length - 1, this.index + 1);
      const delta = this.history[nextIndex];
      const nextState = diff.patch(this.state, delta) as Graph;
      this.index = nextIndex;
      this.state = nextState;
      return clone(nextState);
    } else {
      log.log('reached end');
    }
  }
}
