export type TrackData = Record<string, string | number | boolean>;

type Umami = { track: (name: string, data?: TrackData) => void };

function doNotTrack() {
  const nav = navigator as Navigator & { globalPrivacyControl?: boolean };
  const win = window as unknown as { doNotTrack?: string };
  return nav.doNotTrack === '1' || win.doNotTrack === '1' || nav.globalPrivacyControl === true;
}

// tests, the benchmark and agents driving window.nodarium would skew the numbers
function enabled() {
  if (typeof window === 'undefined' || navigator.webdriver || doNotTrack()) return false;
  return location.hostname !== 'localhost' && location.hostname !== '127.0.0.1';
}

let context: TrackData = {};

/** The build the events come from, sent with every event */
export function setTrackContext(git: Record<string, string> | undefined, version: string) {
  const channel = git?.ref_type === 'tag' ? 'release' : git?.branch || 'unknown';
  context = { channel, version };
}

/**
 * Sends an event to Umami. The data must never hold anything a user typed or made: no graphs,
 * titles, values or error messages, only ids of the public catalog, enums, counts and buckets.
 */
export function track(name: string, data?: TrackData) {
  if (!enabled()) return;
  try {
    (window as unknown as { umami?: Umami }).umami?.track(name, { ...context, ...data });
  } catch {
    // analytics must never break the app
  }
}

/** Ids of the public catalog pass, anything a user dropped or wrote becomes "custom" */
export function publicNodeType(type: string | undefined) {
  if (!type) return 'unknown';
  return /^max\/(core|plantarium|myzel)\/[a-z0-9_-]+$/.test(type) ? type : 'custom';
}

export function msBucket(ms: number) {
  if (ms < 2000) return '<2s';
  if (ms < 5000) return '2-5s';
  if (ms < 10000) return '5-10s';
  return '>10s';
}

export function countBucket(n: number) {
  if (n === 0) return '0';
  if (n < 5) return '1-4';
  if (n < 20) return '5-19';
  if (n < 100) return '20-99';
  return '100+';
}

const counters: Record<string, number> = {};

/** Counts actions too frequent for an event each, they go out as one session-summary */
export function count(key: string, by = 1) {
  counters[key] = (counters[key] ?? 0) + by;
}

function flush() {
  const keys = Object.keys(counters);
  if (keys.length === 0) return;
  const data: TrackData = {};
  for (const key of keys) {
    data[key] = counters[key];
    delete counters[key];
  }
  track('session-summary', data);
}

if (typeof document !== 'undefined') {
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') flush();
  });
}
