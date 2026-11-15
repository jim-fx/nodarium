type EventMap = Record<string, unknown>;
type EventKey<T extends EventMap> = string & keyof T;

export default class EventEmitter<T extends EventMap> {
  private cbs: { [K in keyof T]?: ((data: T[K]) => unknown)[] } = {};

  protected emit<K extends EventKey<T>>(event: K, data: T[K]) {
    this.cbs[event]?.forEach((c) => c(data));
  }

  public on<K extends EventKey<T>>(event: K, cb: (data: T[K]) => unknown) {
    const cbs = (this.cbs[event] ??= []);
    cbs.push(cb);
    return () => {
      const i = cbs.indexOf(cb);
      if (i !== -1) cbs.splice(i, 1);
    };
  }
}
