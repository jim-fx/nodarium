import { beforeEach, expect, test } from 'vitest';
import { clearLogs, createLogger, getLogEntries, LOG_BUFFER_SIZE } from './logger';

beforeEach(() => clearLogs());

test('the log buffer keeps only the newest LOG_BUFFER_SIZE entries', () => {
  const log = createLogger('bound');
  for (let i = 0; i < LOG_BUFFER_SIZE + 10; i++) log.info(i);

  const entries = getLogEntries();
  expect(entries).toHaveLength(LOG_BUFFER_SIZE);
  expect(entries[0].args).toEqual(['10']);
  expect(entries.at(-1)!.args).toEqual([String(LOG_BUFFER_SIZE + 9)]);
});

test('logged typed arrays are stored as a summary, not the array', () => {
  const log = createLogger('summary');
  log.mute();
  log.warn('values', new Int32Array(1_000_000).fill(7), [[Int32Array.of(1, 2)]]);

  const [entry] = getLogEntries();
  expect(entry.args).toEqual([
    'values',
    'Int32Array(1000000) [7, 7, 7, 7, 7, 7, 7, 7, …]',
    '[[Int32Array(2)]]'
  ]);
});

test('muted scopes record only warnings and errors', () => {
  const log = createLogger('muted');
  log.mute();
  log.group('group');
  log.log('Inputs:', [Int32Array.of(1)]);
  log.groupEnd();
  log.warn('warn');

  expect(getLogEntries().map(e => e.level)).toEqual(['warn']);
});

test('values that throw when read are logged without throwing', () => {
  const log = createLogger('throwing');
  log.mute();
  const getter = {
    get value() {
      throw new Error('getter');
    }
  };
  const proxy = new Proxy({}, {
    get() {
      throw new Error('trap');
    },
    ownKeys() {
      throw new Error('trap');
    }
  });
  const revoked = Proxy.revocable({}, {});
  revoked.revoke();

  expect(() => log.warn('values', getter, proxy, revoked.proxy, 1)).not.toThrow();
  expect(getLogEntries()[0].args).toEqual([
    'values',
    '[unloggable object]',
    '[unloggable object]',
    '[unloggable object]',
    '1'
  ]);
});
