import { expect, type Page, test } from '@playwright/test';

test.use({ viewport: { width: 1600, height: 1000 } });

const graph = {
  id: 0,
  version: 19,
  nodes: [
    { id: 1, type: 'max/plantarium/branch', position: [60, 0], props: {} },
    { id: 2, type: 'max/plantarium/gravity', position: [200, 0], props: {} },
    { id: 3, type: 'max/core/output', position: [270, 60], props: {} }
  ],
  edges: [],
  groups: []
};

async function openGraph(page: Page) {
  await page.goto('http://localhost:4173/?tutorial=0', { waitUntil: 'load' });
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(async (g) => {
    await window.nodarium!.ready;
    await window.nodarium!.graph.load(g, { title: 'help mode' });
  }, graph);
  await expect(page.getByRole('group', { name: 'branch, node 1' })).toBeVisible();
}

const header = (page: Page, id: number) =>
  page.locator(`.wrapper[data-node-id="${id}"][data-node-type]`);
const help = (page: Page) => page.locator('.help-wrapper');

test('? turns the help mode on and off, a node with a sketch shows it', async ({ page }) => {
  await openGraph(page);
  await page.mouse.click(5, 990);

  await page.keyboard.press('?');
  await header(page, 1).hover();
  await expect(help(page)).toBeVisible();
  await expect(help(page).locator('[data-node-sketch="max/plantarium/branch"]')).toBeVisible();

  await header(page, 2).hover();
  await expect(help(page)).toContainText('gravity');
  await expect(help(page).locator('[data-node-sketch]')).toHaveCount(0);

  await page.mouse.click(5, 990);
  await page.keyboard.press('?');
  await header(page, 1).hover();
  await expect(help(page)).toHaveCount(0);
});

test('? typed into an input leaves the help mode alone', async ({ page }) => {
  await openGraph(page);
  await page.getByTitle('Rename (F2)').click();
  const name = page.getByRole('textbox', { name: 'Project name' });
  await expect(name).toBeFocused();
  await page.keyboard.press('?');
  // the name was selected, the key replaced it
  await expect(name).toHaveValue('?');
  await page.keyboard.press('Escape');

  await header(page, 1).hover();
  await expect(help(page)).toHaveCount(0);
});
