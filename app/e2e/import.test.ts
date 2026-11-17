import { expect, test } from '@playwright/test';

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

test('an imported graph with mistakes names them and opens', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('planty:nodarium-tutorial:seen', 'e2e');
    localStorage.setItem('app-settings', JSON.stringify({ debug: { advancedMode: true } }));
  });
  await page.goto('http://localhost:4173', { waitUntil: 'load' });

  await page.getByRole('button', { name: 'File', exact: true }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('menuitem', { name: /^Import \.json/ }).click();
  await (await chooser).setFiles({
    name: 'broken.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(graph))
  });

  await expect(page.getByRole('status')).toContainText(
    'The imported graph has 3 issues: stem#1 has no input "thicknes", did you mean "thickness"?'
  );
  await expect(page.locator('div.wrapper[data-node-id]')).toHaveCount(3);

  await page.getByRole('button', { name: 'errors', exact: true }).click();
  for (
    const message of [
      'stem#1 has no input "thicknes", did you mean "thickness"?',
      'output#0 has no input "inputt", did you mean "input"?',
      'noise#2 "scale" takes float, output 0 of stem#1 is path'
    ]
  ) {
    await expect(page.getByText(message, { exact: true }).last()).toBeVisible();
  }
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

test('an imported graph without positions shows its nodes left to right', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('planty:nodarium-tutorial:seen', 'e2e');
  });
  await page.goto('http://localhost:4173', { waitUntil: 'load' });

  await page.getByRole('button', { name: 'File', exact: true }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('menuitem', { name: /^Import \.json/ }).click();
  await (await chooser).setFiles({
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
