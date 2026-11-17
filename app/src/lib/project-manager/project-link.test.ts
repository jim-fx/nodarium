// The links of the app and of `nodarium link` use the encoding of @nodarium/runtime, this checks
// that each side opens what the other one makes.
import { readUrlOptions } from '#lib/url-options.ts';
import { decodeGraph, parseProjectFile } from '@nodarium/runtime';
import { loadTemplate } from '@nodarium/runtime/templates';
import type { Graph } from '@nodarium/types';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { main } from '../../../../packages/runtime/src/cli/main';
import { projectLink } from './project-link';

const nodesDir = fileURLToPath(new URL('../../../static/nodes', import.meta.url));

async function cli(...args: string[]) {
  const out: string[] = [];
  vi.spyOn(console, 'log').mockImplementation((...line) => void out.push(line.join(' ')));
  vi.spyOn(console, 'error').mockImplementation(() => {});
  const code = await main([...args, '--nodes', nodesDir]);
  return { code, out: out.join('\n') };
}

/** What the page does with the address it is opened with */
async function openInApp(link: string) {
  const { graph } = readUrlOptions(new URL(link));
  return parseProjectFile(await decodeGraph(graph!)).graph;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('graph links', () => {
  it('of nodarium link open in the app', async () => {
    const { code, out } = await cli('link', 'fern', '--base', 'http://localhost:4173');
    expect(code).toBe(0);
    const fern = await loadTemplate('fern');
    const opened = await openInApp(out);
    expect(opened.nodes).toEqual(fern.nodes);
    expect(opened.edges).toEqual(fern.edges);
    expect(opened.meta?.title).toBe('fern');
  });

  it('of the app open with nodarium', async () => {
    const project = { ...structuredClone(await loadTemplate('daisy')), id: 4 } as Graph;
    project.meta = { ...project.meta, title: 'My daisy', lastModified: '2026-11-17T10:00:00Z' };
    const link = await projectLink(project, 'https://nodes.max-richter.dev');
    expect(link).toMatch(/^https:\/\/nodes\.max-richter\.dev\/#graph=[\w-]+$/);

    expect(await cli('check', link)).toMatchObject({ code: 0, out: 'my-daisy: no issues' });
    const printed = JSON.parse((await cli('run', link, '--print-graph')).out);
    expect(printed.nodes.map((n: Graph['nodes'][number]) => n.props)).toEqual(
      project.nodes.map((n) => n.props)
    );
    expect(printed.meta).toEqual({ title: 'My daisy', unit: project.meta.unit });
  });
});
