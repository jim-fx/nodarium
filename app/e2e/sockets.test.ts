import { expect, type Page, test } from '@playwright/test';

test.use({ viewport: { width: 1920, height: 1080 } });

// stem and noise side by side, only noise → output is connected
const graph = {
  id: 0,
  version: 13,
  nodes: [
    { id: 1, type: 'max/plantarium/stem', position: [0, 0], props: {} },
    { id: 2, type: 'max/plantarium/noise', position: [40, 0], props: {} },
    { id: 3, type: 'max/plantarium/output', position: [80, 0], props: {} }
  ],
  edges: [[2, 0, 3, 'input']],
  groups: []
};

async function openGraph(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem('planty:nodarium-tutorial:seen', 'e2e');
  });
  await page.goto('http://localhost:4173', { waitUntil: 'load' });

  await page.getByRole('button', { name: 'File', exact: true }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('menuitem', { name: /^Import \.json/ }).click();
  await (await chooser).setFiles({
    name: 'sockets.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(graph))
  });
  await expect(page.getByRole('group', { name: 'noise, node 2' })).toBeVisible();
}

async function savedEdges(page: Page) {
  await page.getByRole('button', { name: 'File', exact: true }).click();
  const download = page.waitForEvent('download');
  await page.getByRole('menuitem', { name: /^Export \.json/ }).click();
  const stream = await (await download).createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(chunk as Buffer);
  return JSON.parse(Buffer.concat(chunks).toString()).graph.edges;
}

// the migration puts a to_geometry (4) between noise and the output
const connected = [[2, 0, 4, 'input'], [4, 0, 3, 'input'], [1, 0, 2, 'plant']];

test('a click on an output and then on an input connects them', async ({ page }) => {
  await openGraph(page);
  const nodes = page.getByRole('group', { name: 'Nodes' });
  const output = nodes.getByRole('button', { name: 'output path of stem' });
  const input = nodes.getByRole('button', { name: 'input plant of noise' });

  await output.click();
  await expect(output).toHaveAttribute('aria-pressed', 'true');
  await input.click();
  await expect(output).toHaveAttribute('aria-pressed', 'false');

  await expect(input).toHaveAccessibleDescription('connected to output path of stem (#1)');
  await expect.poll(() => savedEdges(page)).toEqual(connected);
});

test('Enter on an output and then on an input connects them', async ({ page }) => {
  await openGraph(page);
  const nodes = page.getByRole('group', { name: 'Nodes' });
  const input = nodes.getByRole('button', { name: 'input plant of noise' });

  await nodes.getByRole('button', { name: 'output path of stem' }).focus();
  await page.keyboard.press('Enter');
  for (let i = 0; i < 50; i++) {
    if (await input.evaluate((el) => el === document.activeElement)) break;
    await page.keyboard.press('Tab');
  }
  await expect(input).toBeFocused();
  await page.keyboard.press('Enter');

  await expect(input).toHaveAccessibleDescription('connected to output path of stem (#1)');
  await expect.poll(() => savedEdges(page)).toEqual(connected);
});
