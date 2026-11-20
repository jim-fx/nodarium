import { expect, type Page, test } from '@playwright/test';

const graph = {
  version: 14,
  nodes: [
    { id: 1, type: 'max/plantarium/stem', props: { length: 2, thickness: 0.2 } },
    { id: 2, type: 'max/plantarium/noise' },
    { id: 3, type: 'max/plantarium/output' }
  ],
  edges: [[1, 0, 2, 'plant'], [2, 0, 3, 'input']]
};

async function open(page: Page) {
  await page.goto('http://localhost:4173/?tutorial=0&panel=graph-settings');
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(() => window.nodarium!.ready);
}

const frames = (page: Page) => page.evaluate(() => window.nodarium!.graph.get().frames);
const positions = (page: Page) =>
  page.evaluate(() =>
    Object.fromEntries(window.nodarium!.graph.get().nodes.map((n) => [n.id, n.position!]))
  );

test('a frame is made of the selection, moved by its title, renamed and coloured', async ({ page }) => {
  await open(page);
  await page.evaluate((g) => window.nodarium!.graph.load(g, { title: 'frames' }), graph);
  await page.evaluate(() => window.nodarium!.idle());

  // the editor reacts to the keyboard once it has the focus, `.` fits the nodes into the view
  await page.locator('.graph-wrapper').focus();
  await page.keyboard.press('.');
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Control+j');
  const ids = Object.keys(await positions(page)).map(Number);
  await expect.poll(() => frames(page)).toEqual([{ id: 1, label: 'Frame', nodes: ids }]);

  const title = page.locator('.frame-header .frame-label');
  await expect(title).toHaveText('Frame');
  await expect(title).toBeVisible();
  // the camera glides to the nodes, the title stays where it is once it is there
  let last = '';
  await expect.poll(async () => {
    const at = JSON.stringify(await title.boundingBox());
    const settled = at === last;
    last = at;
    return settled;
  }, { intervals: [100] }).toBe(true);

  const before = await positions(page);
  const box = (await title.boundingBox())!;
  await page.mouse.move(box.x + 10, box.y + 5);
  await page.mouse.down();
  await page.mouse.move(box.x + 60, box.y + 45, { steps: 5 });
  await page.mouse.up();
  const after = await positions(page);
  const dx = after[1][0] - before[1][0];
  const dy = after[1][1] - before[1][1];
  expect(dx).toBeGreaterThan(0);
  expect(dy).toBeGreaterThan(0);
  for (const id of ids) {
    expect(after[id]).toEqual([before[id][0] + dx, before[id][1] + dy]);
  }

  await title.dblclick();
  const input = page.getByRole('textbox', { name: 'Frame title' });
  await expect(input).toBeFocused();
  await input.fill('Stem');
  await input.press('Enter');
  await expect(title).toHaveText('Stem');

  await page.getByRole('radio', { name: 'brown' }).click();
  await expect.poll(() => frames(page)).toEqual([
    { id: 1, label: 'Stem', color: 'brown', nodes: ids }
  ]);

  await page.reload();
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(() => window.nodarium!.ready);
  expect(await frames(page)).toEqual([{ id: 1, label: 'Stem', color: 'brown', nodes: ids }]);
  await expect(page.locator('.frame-header .frame-label')).toHaveText('Stem');
});

test('graph.load and graph.get keep the frames of a graph', async ({ page }) => {
  await open(page);
  const framed = { ...graph, frames: [{ id: 4, label: 'Stem', color: 'green', nodes: [1, 2] }] };
  await page.evaluate((g) => window.nodarium!.graph.load(g, { title: 'frames' }), framed);
  expect(await frames(page)).toEqual(framed.frames);
  await expect(page.locator('.frame-header .frame-label')).toHaveText('Stem');
});
