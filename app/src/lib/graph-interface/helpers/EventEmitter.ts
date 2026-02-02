import throttle from '$lib/helpers/throttle';

type EventMap = Record<string, unknown>;
type EventKey<T extends EventMap> = string & keyof T;
type EventReceiver<T> = (params: T, stuff?: Record<string, unknown>) => unknown;

export default class EventEmitter<
  T extends EventMap = { [key: string]: unknown }
> {
  index = 0;
  public eventMap: T = {} as T;
  constructor() {}

  private cbs: { [key: string]: ((data?: unknown) => unknown)[] } = {};
  private cbsOnce: { [key: string]: ((data?: unknown) => unknown)[] } = {};

  /**
   * Emit an event with optional data to all the listeners
   * @param {string} event Name of the event to emit
   * @param data Data to send along
   */
  public emit(event: string, data?: unknown) {
    if (event in this.cbs) {
      this.cbs[event].forEach((c) => c(data));
    }
    if (event in this.cbsOnce) {
      this.cbsOnce[event].forEach((c) => c(data));
      delete this.cbsOnce[event];
    }
  }

  public on<K extends EventKey<T>>(
    event: K,
    cb: EventReceiver<T[K]>,
    throttleTimer = 0
  ) {
    if (throttleTimer > 0) cb = throttle(cb, throttleTimer);
    const cbs = Object.assign(this.cbs, {
      [event]: [...(this.cbs[event] || []), cb]
    });
    this.cbs = cbs;

    // console.log('New EventEmitter ', this.constructor.name);
    return () => {
      this.cbs[event]?.splice(cbs[event].indexOf(cb), 1);
    };
  }

  /**
   * Register a special listener which only gets called once
   * @param {string} event Name of the event to listen to
   * @param {function} cb Listener, gets called everytime the event is emitted
   * @returns {function} Returns a function which removes the listener when called
   */
  public once<K extends EventKey<T>>(
    event: K,
    cb: EventReceiver<T[K]>
  ): () => void {
    const cbsOnce = Object.assign(this.cbsOnce, {
      [event]: [...(this.cbsOnce[event] || []), cb]
    });
    this.cbsOnce = cbsOnce;

    return () => {
      cbsOnce[event]?.splice(cbsOnce[event].indexOf(cb), 1);
    };
  }

  public destroyEventEmitter() {
    Object.keys(this.cbs).forEach((key) => {
      delete this.cbs[key];
    });
    Object.keys(this.cbsOnce).forEach((key) => delete this.cbsOnce[key]);
    this.cbs = {};
    this.cbsOnce = {};
  }
}
