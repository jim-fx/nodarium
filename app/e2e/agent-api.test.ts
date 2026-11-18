import { expect, type Page, test } from '@playwright/test';
import type { Graph } from '../src/lib/agent-api/types.ts';

// one page for all tests, every test opens the graph it needs as a new project
test.describe.configure({ mode: 'serial' });

let page: Page;

const plant = {
  version: 18,
  nodes: [
    { id: 1, type: 'max/plantarium/stem', props: { amount: 4, length: 0.6 } },
    { id: 2, type: 'max/plantarium/noise', props: { strength: 0.4 } },
    { id: 4, type: 'max/plantarium/to_geometry' },
    { id: 3, type: 'max/core/output' }
  ],
  edges: [[1, 0, 2, 'plant'], [2, 0, 4, 'input'], [4, 0, 3, 'input']]
};

function loadPlant() {
  return page.evaluate((g) => window.nodarium!.graph.load(g), plant);
}

function nodeOf(graph: Graph, id: number) {
  return graph.nodes.find((n) => n.id === id);
}

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
  await page.goto('http://localhost:4173/?tutorial=0');
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
});

test.afterAll(() => page.close());

test('ready: the first build is done and every node is known', async () => {
  const state = await page.evaluate(async () => {
    const nodarium = window.nodarium!;
    await nodarium.ready;
    const ids = nodarium.nodes.list().map((d) => d.id);
    return {
      version: nodarium.version,
      app: nodarium.app,
      built: nodarium.lastResult() !== undefined,
      core: ids.filter((id) => id.startsWith('max/core/')).length,
      plantarium: ids.filter((id) => id.startsWith('max/plantarium/')).length,
      myzel: ids.filter((id) => id.startsWith('max/myzel/')).length
    };
  });
  expect(state.version).toBe(1);
  expect(state.app.version).toMatch(/^\d+\.\d+\.\d+$/);
  expect(typeof state.app.sha).toBe('string');
  expect(state.built).toBe(true);
  expect(state.core).toBe(12);
  expect(state.plantarium).toBeGreaterThan(10);
  expect(state.myzel).toBeGreaterThan(10);
});

test('graph.load opens a new project and builds it', async () => {
  const before = await page.evaluate(() => window.nodarium!.graph.get().id);
  const result = await page.evaluate(
    (g) => window.nodarium!.graph.load(g, { title: 'agent test' }),
    plant
  );
  expect(result).toMatchObject({ ok: true, issues: [] });
  expect(result.vertices).toBeGreaterThan(0);
  expect(result.bounds?.min).toHaveLength(3);
  const graph = await page.evaluate(() => window.nodarium!.graph.get());
  expect(graph.id).not.toBe(before);
  expect(graph.meta?.title).toBe('agent test');

  await expect(page.evaluate(() => window.nodarium!.graph.load(42))).rejects.toThrow(
    /Invalid project file/
  );
});

test('graph.loadTemplate opens a template as a new project', async () => {
  const result = await page.evaluate(() => window.nodarium!.graph.loadTemplate('fern'));
  expect(result.ok).toBe(true);
  expect(result.faces).toBeGreaterThan(1000);
  expect(await page.evaluate(() => window.nodarium!.graph.get().meta?.title)).toBe('Fern');
  await expect(page.evaluate(() => window.nodarium!.graph.loadTemplate('nope'))).rejects.toThrow(
    /There is no template "nope"/
  );
});

test('graph.get is a copy of the open graph', async () => {
  await loadPlant();
  const lengths = await page.evaluate(() => {
    const graph = window.nodarium!.graph.get();
    graph.nodes.pop();
    return [graph.nodes.length, window.nodarium!.graph.get().nodes.length];
  });
  expect(lengths).toEqual([3, 4]);
});

test('graph.addNode adds a node, one undo step', async () => {
  await loadPlant();
  const id = await page.evaluate(() =>
    window.nodarium!.graph.addNode('max/plantarium/noise', { strength: 0.2 })
  );
  expect(nodeOf(await page.evaluate(() => window.nodarium!.graph.get()), id)?.props).toEqual({
    strength: 0.2
  });
  await expect(page.locator(`div.wrapper[data-node-id="${id}"]`)).toBeAttached();

  await page.evaluate(() => window.nodarium!.graph.undo());
  expect(nodeOf(await page.evaluate(() => window.nodarium!.graph.get()), id)).toBeUndefined();

  await expect(page.evaluate(() => window.nodarium!.graph.addNode('max/plantarium/nope')))
    .rejects.toThrow(/no node type "max\/plantarium\/nope"/);
  await expect(
    page.evaluate(() => window.nodarium!.graph.addNode('max/plantarium/noise', { strenght: 1 }))
  ).rejects.toThrow(/has no input "strenght"/);
});

test('graph.removeNode removes a node and its edges', async () => {
  await loadPlant();
  const result = await page.evaluate(async () => {
    window.nodarium!.graph.removeNode(2);
    await window.nodarium!.idle();
    return { graph: window.nodarium!.graph.get(), result: window.nodarium!.lastResult()! };
  });
  expect(nodeOf(result.graph, 2)).toBeUndefined();
  expect(result.graph.edges).toEqual([[4, 0, 3, 'input']]);
  expect(result.result.vertices).toBe(0);
  await expect(page.evaluate(() => window.nodarium!.graph.removeNode(99))).rejects.toThrow(
    /no node 99/
  );
});

test('graph.connect connects an output to an input', async () => {
  await loadPlant();
  const edges = await page.evaluate(async () => {
    const { graph } = window.nodarium!;
    graph.disconnect(4, 'input');
    graph.connect(1, 0, 4, 'input');
    await window.nodarium!.idle();
    return graph.get().edges;
  });
  expect(edges).toContainEqual([1, 0, 4, 'input']);
  expect(edges).not.toContainEqual([2, 0, 4, 'input']);

  await expect(page.evaluate(() => window.nodarium!.graph.connect(1, 0, 2, 'scale'))).rejects
    .toThrow('"scale" of noise#2 takes float, output 0 of stem#1 is path');
  await expect(page.evaluate(() => window.nodarium!.graph.connect(2, 0, 1, 'plant'))).rejects
    .toThrow(/has no input "plant"/);
  await expect(page.evaluate(() => {
    const { graph } = window.nodarium!;
    const noise = graph.addNode('max/plantarium/noise');
    graph.connect(2, 0, noise, 'plant');
    graph.connect(noise, 0, 2, 'plant');
  })).rejects.toThrow(/would make a cycle/);
});

test('graph.disconnect removes the edge into an input', async () => {
  await loadPlant();
  const result = await page.evaluate(async () => {
    window.nodarium!.graph.disconnect(4, 'input');
    await window.nodarium!.idle();
    return { edges: window.nodarium!.graph.get().edges, result: window.nodarium!.lastResult()! };
  });
  expect(result.edges).toEqual([[1, 0, 2, 'plant'], [4, 0, 3, 'input']]);
  expect(result.result.vertices).toBe(0);
  expect(result.result.issues.map((i) => i.code)).toContain('unreachable-node');
});

test('graph.setProp changes an input and builds again', async () => {
  const before = await loadPlant();
  const after = await page.evaluate(async () => {
    window.nodarium!.graph.setProp(1, 'length', 1.2);
    await window.nodarium!.idle();
    return {
      props: window.nodarium!.graph.get().nodes.find((n) => n.id === 1)?.props,
      result: window.nodarium!.lastResult()!
    };
  });
  expect(after.props).toMatchObject({ length: 1.2 });
  expect(after.result.bounds!.max[1]).toBeGreaterThan(before.bounds!.max[1] + 0.3);
  await expect(page.evaluate(() => window.nodarium!.graph.setProp(1, 'lenght', 1))).rejects
    .toThrow(/stem has no input "lenght"/);
});

test('graph.setSettings changes the settings of the graph', async () => {
  const before = await loadPlant();
  const after = await page.evaluate(async () => {
    window.nodarium!.graph.setSettings({ 'resolution.circle': 6 });
    await window.nodarium!.idle();
    return {
      settings: window.nodarium!.graph.get().settings,
      result: window.nodarium!.lastResult()!
    };
  });
  expect(after.settings?.['resolution.circle']).toBe(6);
  expect(after.result.vertices).toBeLessThan(before.vertices);
  await expect(page.evaluate(() => window.nodarium!.graph.setSettings({ nope: 1 }))).rejects
    .toThrow(/no setting "nope"/);
});

test('graph.format lays the nodes out left to right', async () => {
  await loadPlant();
  const x = await page.evaluate(() => {
    window.nodarium!.graph.format();
    const graph = window.nodarium!.graph.get();
    return [1, 2, 4, 3].map((id) => graph.nodes.find((n) => n.id === id)!.position![0]);
  });
  expect(x[0]).toBeLessThan(x[1]);
  expect(x[1]).toBeLessThan(x[2]);
  expect(x[2]).toBeLessThan(x[3]);
});

test('graph.undo and graph.redo step through the changes, one step per change', async () => {
  await loadPlant();
  const lengths = await page.evaluate(() => {
    const { graph } = window.nodarium!;
    const length = () => graph.get().nodes.find((n) => n.id === 1)?.props?.length;
    graph.setProp(1, 'length', 0.8);
    graph.setProp(1, 'length', 0.9);
    const set = length();
    graph.undo();
    const undone = length();
    graph.undo();
    const undoneTwice = length();
    graph.redo();
    return [set, undone, undoneTwice, length()];
  });
  expect(lengths).toEqual([0.9, 0.8, 0.6, 0.8]);
});

test('nodes.list and nodes.get describe the nodes', async () => {
  const stem = await page.evaluate(() => window.nodarium!.nodes.get('max/plantarium/stem'));
  expect(stem?.inputs).toHaveProperty('length');
  expect(await page.evaluate(() => window.nodarium!.nodes.get('max/plantarium/nope'))).toBe(
    undefined
  );
  const listed = await page.evaluate(() => window.nodarium!.nodes.list().map((d) => d.id));
  expect(listed).toContain('max/plantarium/stem');
});

test('run builds with a seed, the same seed gives the same result', async () => {
  await page.evaluate(() => window.nodarium!.graph.loadTemplate('default-plant'));
  const [one, two, again, seed] = await page.evaluate(async () => {
    const nodarium = window.nodarium!;
    return [
      await nodarium.run({ seed: 1 }),
      await nodarium.run({ seed: 2 }),
      await nodarium.run({ seed: 1 }),
      nodarium.graph.get().settings?.seed
    ] as const;
  });
  expect(again.bounds).toEqual(one.bounds);
  expect(two.bounds).not.toEqual(one.bounds);
  expect(seed).toBe(1);
});

test('lastResult is the result of the last build', async () => {
  const result = await loadPlant();
  expect(await page.evaluate(() => window.nodarium!.lastResult())).toEqual(result);
});

test('benchmark builds with random seeds and leaves the graph as it was', async () => {
  await page.evaluate(() => window.nodarium!.graph.loadTemplate('default-plant'));
  const { before, benchmark, after, result } = await page.evaluate(async () => {
    const nodarium = window.nodarium!;
    const before = nodarium.graph.get();
    const benchmark = await nodarium.benchmark({ runs: 3, warmups: 1 });
    return { before, benchmark, after: nodarium.graph.get(), result: nodarium.lastResult() };
  });
  expect(benchmark.samples).toHaveLength(3);
  expect(benchmark.median).toBeGreaterThanOrEqual(0);
  expect(benchmark.stdev).toBeGreaterThanOrEqual(0);
  expect(after.settings).toEqual(before.settings);
  expect(result?.ok).toBe(true);
});

test('viewer.screenshot takes a PNG of the views next to each other', async () => {
  await loadPlant();
  const image = await page.evaluate(async () => {
    const blob = await window.nodarium!.viewer.screenshot({ size: 256, views: ['front', 'top'] });
    const bitmap = await createImageBitmap(blob);
    return { type: blob.type, width: bitmap.width, height: bitmap.height };
  });
  expect(image).toEqual({ type: 'image/png', width: 512, height: 256 });
  await expect(
    page.evaluate(() =>
      window.nodarium!.viewer.screenshot({ views: ['below' as unknown as 'top'] })
    )
  ).rejects.toThrow(/views takes some of default, front, side, top/);
});

test('idle waits for a slow build', async () => {
  await page.evaluate(() => window.nodarium!.graph.loadTemplate('champignon-groomed'));
  const { waited, before, after } = await page.evaluate(async () => {
    const nodarium = window.nodarium!;
    const before = nodarium.lastResult()!;
    nodarium.graph.setSettings({ seed: 4242 });
    const start = performance.now();
    await nodarium.idle();
    return { waited: performance.now() - start, before, after: nodarium.lastResult()! };
  });
  expect(waited).toBeGreaterThan(after.ms.total / 2);
  expect(after).not.toEqual(before);
  await expect(page.getByRole('status').filter({ hasText: 'vertices' })).toBeVisible();
});

for (const template of ['default-plant', 'champignon-groomed']) {
  test(`idle waits for the last of two quick changes (${template})`, async () => {
    await page.evaluate((id) => window.nodarium!.graph.loadTemplate(id), template);
    const quick = await page.evaluate(async () => {
      const nodarium = window.nodarium!;
      nodarium.graph.setSettings({ seed: 11 });
      nodarium.graph.setSettings({ seed: 12 });
      await nodarium.idle();
      return nodarium.lastResult()!;
    });
    // the same graph, built on its own
    const reference = await page.evaluate(async () => {
      const nodarium = window.nodarium!;
      return nodarium.graph.load(nodarium.graph.get());
    });
    expect(quick.bounds).toEqual(reference.bounds);
    expect(quick.vertices).toBe(reference.vertices);
  });
}
