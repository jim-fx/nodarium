import { expect, type Page, test } from '@playwright/test';

test.use({ viewport: { width: 1920, height: 1080 } });

// stem → noise → output, with three mistakes
const graph = {
  id: 0,
  version: 13,
  settings: { 'resolution.circle': 12, 'resolution.curve': 16 },
  nodes: [
    { id: 0, type: 'max/plantarium/output', position: [200, 0], props: {} },
    { id: 1, type: 'max/plantarium/stem', position: [0, 0], props: { thicknes: 0.1 } },
    { id: 2, type: 'max/plantarium/noise', position: [100, 0], props: { strength: 0.5 } }
  ],
  edges: [[1, 0, 2, 'plant'], [2, 0, 0, 'inputt'], [1, 0, 2, 'scale']],
  groups: []
};

const mistakes = [
  'stem#1 has no input "thicknes", did you mean "thickness"?',
  'output#0 has no input "inputt", did you mean "input"?',
  'noise#2 "scale" takes float, output 0 of stem#1 is path'
];

async function open(page: Page) {
  await page.goto('http://localhost:4173/?tutorial=0');
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(() => window.nodarium!.ready);
}

test('an imported graph with mistakes names them and opens', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('app-settings', JSON.stringify({ debug: { advancedMode: true } }));
  });
  await open(page);

  await page.getByRole('button', { name: 'File', exact: true }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('menuitem', { name: /^Import \.json/ }).click();
  await (await chooser).setFiles({
    name: 'broken.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(graph))
  });

  await expect(page.getByRole('status').filter({ hasText: 'The imported graph' })).toContainText(
    `The imported graph has 3 issues: ${mistakes[0]}`
  );
  await expect(page.locator('div.wrapper[data-node-id]')).toHaveCount(3);

  await page.getByRole('button', { name: 'errors', exact: true }).click();
  for (const message of mistakes) {
    await expect(page.getByText(message, { exact: true }).last()).toBeVisible();
  }
});

test('graph.load reports the mistakes an import reports', async ({ page }) => {
  await open(page);
  const result = await page.evaluate((g) => window.nodarium!.graph.load(g), graph);
  expect(result.ok).toBe(false);
  expect(result.issues.map((issue) => issue.message)).toEqual(mistakes);
  await expect(page.getByRole('status').filter({ hasText: 'The imported graph' })).toBeVisible();
});

// written by hand: no positions, the editor places the nodes in columns
const unplaced = {
  version: 13,
  nodes: [
    { id: 1, type: 'max/plantarium/stem', props: { length: 0.5 } },
    { id: 2, type: 'max/plantarium/noise' },
    { id: 3, type: 'max/plantarium/output' }
  ],
  edges: [[1, 0, 2, 'plant'], [2, 0, 3, 'input']]
};

test('a file given to the import input opens, its nodes left to right', async ({ page }) => {
  await open(page);

  // the input a tool can upload to without the file dialog of the menu
  await page.getByRole('button', { name: 'Import a project file' }).setInputFiles({
    name: 'unplaced.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(unplaced))
  });

  const node = (id: number) => page.locator(`div.wrapper[data-node-id="${id}"]`);
  // the camera glides to the placed nodes
  const leftToRight = async () => {
    const boxes = await Promise.all([1, 2, 3].map((id) => node(id).boundingBox()));
    return boxes.map((box) => box?.x ?? NaN);
  };
  await expect.poll(async () => {
    const [stem, noise, output] = await leftToRight();
    return stem < noise && noise < output;
  }).toBe(true);
  for (const id of [1, 2, 3]) await expect(node(id)).toBeVisible();
});
