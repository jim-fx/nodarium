import { expect, type Page, test } from '@playwright/test';
import { readFileSync } from 'node:fs';

test.use({ viewport: { width: 1920, height: 1080 } });

// the float node under an id the site doesn't serve, the definition is JSON in a custom section
const elsewhere = 'drop/elsewhere/float';
const droppedWasm = Buffer.from(
  readFileSync('static/nodes/max/plantarium/float.wasm')
    .toString('latin1')
    .replaceAll('max/plantarium/float', elsewhere),
  'latin1'
);

const plant = {
  version: 13,
  nodes: [
    { id: 1, type: 'max/plantarium/stem', props: {} },
    { id: 2, type: 'max/plantarium/output' }
  ],
  edges: [[1, 0, 2, 'input']]
};

async function open(page: Page) {
  await page.goto('http://localhost:4173/?tutorial=0');
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(() => window.nodarium!.ready);
}

function drop(page: Page, name: string, bytes: Buffer) {
  return page.evaluate(({ name, bytes }) => {
    const target = document.querySelector('.graph-wrapper')!;
    const rect = target.getBoundingClientRect();
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([Uint8Array.from(bytes)], name, { type: 'application/wasm' }));
    target.dispatchEvent(
      new DragEvent('drop', {
        dataTransfer,
        bubbles: true,
        cancelable: true,
        clientX: rect.x + rect.width / 2,
        clientY: rect.y + rect.height / 2
      })
    );
  }, { name, bytes: [...bytes] });
}

function droppedNode(page: Page) {
  return page.evaluate(
    (type) => window.nodarium!.graph.get().nodes.find((n) => n.type === type)?.id,
    elsewhere
  );
}

function build(page: Page) {
  return page.evaluate(async () => {
    await window.nodarium!.idle();
    const result = window.nodarium!.lastResult()!;
    return { ok: result.ok, error: result.error?.message, height: result.bounds?.max[1] };
  });
}

test('a dropped wasm of a node the site has not builds in the worker, also after a reload', async ({ page }) => {
  await open(page);
  await page.evaluate((g) => window.nodarium!.graph.load(g, { title: 'drop test' }), plant);
  expect((await build(page)).height).toBeCloseTo(5, 0);

  await drop(page, 'elsewhere.wasm', droppedWasm);
  await expect.poll(() => droppedNode(page)).toBeDefined();
  const id = (await droppedNode(page))!;
  await page.evaluate((id) => {
    window.nodarium!.graph.connect(id, 0, 1, 'length');
    window.nodarium!.graph.setProp(id, 'value', 3);
  }, id);
  const after = await build(page);
  expect(after.error).toBeUndefined();
  expect(after.ok).toBe(true);
  expect(after.height).toBeCloseTo(3, 0);

  await page.reload();
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(() => window.nodarium!.ready);
  expect(await droppedNode(page)).toBe(id);
  expect(await build(page)).toEqual(after);
});

test('a dropped file that is no node shows a toast and changes nothing', async ({ page }) => {
  await open(page);
  await page.evaluate((g) => window.nodarium!.graph.load(g, { title: 'drop test' }), plant);
  const before = await page.evaluate(() => window.nodarium!.graph.get().nodes);
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));

  await drop(page, 'noise.wasm', Buffer.from([1, 2, 3, 4, 5, 6, 7, 8, 9]));

  await expect(page.getByRole('status').filter({ hasText: 'Failed to load node' })).toHaveText(
    'Failed to load node: invalid file'
  );
  expect(await page.evaluate(() => window.nodarium!.graph.get().nodes)).toEqual(before);
  expect((await build(page)).ok).toBe(true);
  expect(errors).toEqual([]);
});
