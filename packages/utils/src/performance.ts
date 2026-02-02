export type PerformanceData = Record<string, number[]>[];

export interface PerformanceStore {
  startRun(): void;
  stopRun(): void;
  addPoint(name: string, value?: number): void;
  endPoint(name?: string): void;
  mergeData(data: PerformanceData[number]): void;
  get: () => PerformanceData;
  subscribe: (cb: (v: PerformanceData) => void) => () => void;
}

export function createPerformanceStore(): PerformanceStore {
  let data: PerformanceData = [];

  let currentRun: Record<string, number[]> | undefined;
  let temp: Record<string, number> | undefined;
  let lastPoint: string | undefined;

  const listeners: ((v: PerformanceData) => void)[] = [];
  function subscribe(cb: (v: PerformanceData) => void) {
    listeners.push(cb);
    return () => {
      const i = listeners.indexOf(cb);
      if (i > -1) listeners.splice(i, 1);
    };
  }

  function set(v: PerformanceData) {
    listeners.forEach((l) => l(v));
  }

  function startRun() {
    if (currentRun) return;
    currentRun = {};
    lastPoint = undefined;
    temp = {
      start: performance.now()
    };
  }

  function stopRun() {
    if (currentRun && temp) {
      currentRun['total'] = [performance.now() - temp.start];
      data.push(currentRun);
      data = data.slice(-100);
      currentRun = undefined;
      temp = undefined;
      if (set) set(data);
    }
  }

  function addPoint(name: string, value?: number) {
    if (!currentRun) return;
    if (value === undefined) {
      if (temp) {
        lastPoint = name;
        temp[name] = performance.now();
      }
    } else {
      currentRun[name] = currentRun[name] || [];
      currentRun[name].push(value);
    }
  }

  function get() {
    return data;
  }

  function mergeData(newData: PerformanceData[number]) {
    let r = currentRun;
    if (!r) return;

    Object.keys(newData).forEach((name) => {
      if (name in r) {
        r[name].push(...newData[name]);
      } else {
        r[name] = newData[name];
      }
    });
  }

  function endPoint(name = lastPoint) {
    if (name === lastPoint) lastPoint = undefined;
    if (name && currentRun && temp && name in temp) {
      currentRun[name] = currentRun[name] || [];
      currentRun[name].push(performance.now() - temp[name]);
      delete temp[name];
    }
  }

  return {
    subscribe,
    startRun,
    stopRun,
    addPoint,
    endPoint,
    mergeData,
    get
  };
}
