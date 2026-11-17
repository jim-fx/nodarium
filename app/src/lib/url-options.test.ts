import { describe, expect, it } from 'vitest';
import { readUrlOptions, withoutUrlOptions } from './url-options';

const url = (path: string) => new URL(path, 'https://nodes.max-richter.dev');

describe('readUrlOptions', () => {
  it('lets the tutorial start without parameters', () => {
    expect(readUrlOptions(url('/'))).toEqual({ tutorial: true });
  });

  it('keeps the tutorial closed with tutorial=0', () => {
    expect(readUrlOptions(url('/?tutorial=0')).tutorial).toBe(false);
    expect(readUrlOptions(url('/?foo=1&tutorial=0')).tutorial).toBe(false);
  });

  it('takes any other value as the default', () => {
    expect(readUrlOptions(url('/?tutorial=1')).tutorial).toBe(true);
    expect(readUrlOptions(url('/?tutorial')).tutorial).toBe(true);
  });

  it('reads the template, the seed, the panel and the graph of the fragment', () => {
    expect(readUrlOptions(url('/?template=fern&seed=7&panel=graph-settings#graph=abc'))).toEqual({
      tutorial: true,
      template: 'fern',
      seed: 7,
      panel: 'graph-settings',
      graph: 'abc'
    });
  });

  it('leaves out a seed that is no whole number', () => {
    for (const seed of ['-1', '1.5', 'x', '', '12345678901']) {
      expect(readUrlOptions(url(`/?template=fern&seed=${seed}`)).seed).toBeUndefined();
    }
  });

  it('takes a graph only from the fragment', () => {
    expect(readUrlOptions(url('/?graph=abc')).graph).toBeUndefined();
  });
});

describe('withoutUrlOptions', () => {
  it('removes the options and the graph, keeps the rest', () => {
    expect(withoutUrlOptions(url('/?template=fern&seed=7&tutorial=0&panel=general'))).toBe('');
    expect(withoutUrlOptions(url('/?tutorial=0&foo=1#graph=abc'))).toBe('?foo=1');
    expect(withoutUrlOptions(url('/#other'))).toBeUndefined();
    expect(withoutUrlOptions(url('/?foo=1'))).toBeUndefined();
  });
});
