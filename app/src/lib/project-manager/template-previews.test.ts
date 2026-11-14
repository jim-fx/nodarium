import type { Template } from '@nodarium/runtime/templates';
import type { Graph } from '@nodarium/types';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  loadTemplatePreviews,
  selectStaticPreviews,
  type TemplatePreview
} from './template-previews';

function template(id: string): Template & { load: ReturnType<typeof vi.fn> } {
  const graph = { id: 0, nodes: [], edges: [], meta: { title: id } } as unknown as Graph;
  return { id, name: id, category: 'Plants', unit: 'm', load: vi.fn(async () => graph) };
}

async function collect(
  templates: Template[],
  staticPreviews: Record<string, string>,
  render: (graph: Graph) => Promise<string | undefined>
) {
  const previews: Record<string, TemplatePreview> = {};
  await loadTemplatePreviews(templates, staticPreviews, render, (id, preview) => {
    previews[id] = preview;
  });
  return previews;
}

describe('selectStaticPreviews', () => {
  it('uses a preview only while its hash matches the template', () => {
    const urls = selectStaticPreviews(
      { fern: 'aaa', palm: 'old', willow: 'ccc' },
      { fern: 'aaa', palm: 'new', daisy: 'ddd' }
    );
    expect(urls).toEqual({ fern: '/template-previews/fern.webp?v=aaa' });
  });
});

describe('loadTemplatePreviews', () => {
  afterEach(() => vi.restoreAllMocks());

  it('builds nothing when every template has a static preview', async () => {
    const templates = [template('fern'), template('palm')];
    const render = vi.fn();
    const previews = await collect(templates, { fern: '/f.webp', palm: '/p.webp' }, render);
    expect(previews).toEqual({ fern: '/f.webp', palm: '/p.webp' });
    expect(render).not.toHaveBeenCalled();
    expect(templates.every((t) => t.load.mock.calls.length === 0)).toBe(true);
  });

  it('builds the templates without a static preview live', async () => {
    const templates = [template('fern'), template('palm')];
    const render = vi.fn(async (graph: Graph) => `blob:${graph.meta?.title}`);
    const previews = await collect(templates, { fern: '/f.webp' }, render);
    expect(previews).toEqual({ fern: '/f.webp', palm: 'blob:palm' });
    expect(render).toHaveBeenCalledTimes(1);
  });

  it('reports a failed build as null, logs it once and keeps building the others', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const templates = [template('fern'), template('porcini'), template('palm')];
    const render = vi.fn(async (graph: Graph) => {
      if (graph.meta?.title === 'porcini') {
        throw new Error('Out of memory: Cannot allocate Wasm memory for new instance');
      }
      return `blob:${graph.meta?.title}`;
    });
    const previews = await collect(templates, {}, render);
    expect(previews).toEqual({ fern: 'blob:fern', porcini: null, palm: 'blob:palm' });
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it('reports a template that fails to load or has no geometry as null', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    const broken = template('broken');
    broken.load.mockRejectedValue(new Error('chunk failed'));
    const previews = await collect([broken, template('empty')], {}, async () => undefined);
    expect(previews).toEqual({ broken: null, empty: null });
  });
});
