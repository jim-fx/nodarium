import { describe, expect, it } from 'vitest';
import fern from '../templates/fern.json';
import flyAgaric from '../templates/fly-agaric.json';
import { decodeGraph, encodeGraph, graphLink, graphLinkData } from './graph-link';

async function deflate(text: string) {
  const stream = new Blob([text]).stream().pipeThrough(new CompressionStream('deflate-raw'));
  const bytes = new Uint8Array(await new Response(stream).arrayBuffer());
  return btoa(String.fromCharCode(...bytes)).replace(/\+/g, '-').replace(/\//g, '_')
    .replace(/=+$/, '');
}

describe('graph links', () => {
  it('give back the graph they were made of', async () => {
    for (const graph of [fern, flyAgaric]) {
      const data = await encodeGraph(graph);
      expect(data).toMatch(/^[\w-]+$/);
      expect(await decodeGraph(data)).toEqual(graph);
    }
  });

  it('are a fraction of the JSON', async () => {
    const data = await encodeGraph(flyAgaric);
    expect(data.length).toBeLessThan(JSON.stringify(flyAgaric).length / 2);
  });

  it('point at the app, another one with base', async () => {
    const link = await graphLink(fern);
    expect(link).toMatch(/^https:\/\/nodes\.max-richter\.dev\/#graph=[\w-]+$/);
    expect(await graphLink(fern, 'http://localhost:4173/')).toMatch(
      /^http:\/\/localhost:4173\/#graph=/
    );
    expect(await decodeGraph(graphLinkData(link)!)).toEqual(fern);
  });

  it('find the data in a fragment with other parameters', () => {
    expect(graphLinkData('https://x.dev/?seed=2#graph=abc&foo=1')).toBe('abc');
    expect(graphLinkData('#graph=abc')).toBe('abc');
    expect(graphLinkData('https://x.dev/?graph=abc')).toBeUndefined();
    expect(graphLinkData('https://x.dev/#other')).toBeUndefined();
  });

  it('say what is wrong with a broken link', async () => {
    const data = await encodeGraph(fern);
    await expect(decodeGraph(data.slice(0, data.length / 2))).rejects.toThrow(
      'the graph in the link is broken, it may be cut off'
    );
    await expect(decodeGraph('a+b/c')).rejects.toThrow('characters a graph link never has');
    await expect(decodeGraph(await deflate('not json'))).rejects.toThrow('the link holds no JSON');
  });

  it('stop unpacking a graph larger than the limit', async () => {
    // four megabytes of spaces deflate to a few kilobytes
    const bomb = await deflate(`{"nodes":[${' '.repeat(4 * 1024 * 1024)}]}`);
    expect(bomb.length).toBeLessThan(10_000);
    await expect(decodeGraph(bomb)).rejects.toThrow('larger than 1 MB');
    await expect(decodeGraph(await encodeGraph(fern), 3000)).rejects.toThrow('larger than');
  });
});
