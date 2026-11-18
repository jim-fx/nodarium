import { expect, type Page, test } from '@playwright/test';

test.use({ viewport: { width: 1920, height: 1080 } });

// a plant graph, the menu offers the nodes of every system in it all the same
const graph = {
  id: 0,
  version: 13,
  nodes: [
    { id: 1, type: 'max/plantarium/stem', position: [0, 0], props: {} },
    { id: 2, type: 'max/plantarium/output', position: [80, 0], props: {} }
  ],
  edges: [[1, 0, 2, 'input']],
  groups: []
};

async function openGraph(page: Page) {
  await page.goto('http://localhost:4173/?tutorial=0', { waitUntil: 'load' });
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(async (g) => {
    await window.nodarium!.ready;
    await window.nodarium!.graph.load(g, { title: 'add menu' });
  }, graph);
  await expect(page.getByRole('group', { name: 'stem, node 1' })).toBeVisible();
}

async function openMenu(page: Page) {
  const stem = await page.getByRole('group', { name: 'stem, node 1' }).boundingBox();
  const [x, y] = [stem!.x + stem!.width + 40, stem!.y + 10];
  // the editor reacts to the keyboard and to Ctrl only once it has the focus
  await page.mouse.click(x, y);
  await page.keyboard.press('Shift+A');
  const search = page.getByRole('searchbox', { name: 'Search for a node type' });
  await expect(search).toBeFocused();
  return search;
}

const menu = (page: Page) => page.getByRole('tree', { name: 'Node types' });

test('Shift+A lists the nodes of every system under its heading', async ({ page }) => {
  await openGraph(page);
  await openMenu(page);

  const sections = menu(page).getByRole('group');
  await expect(sections).toHaveCount(3);
  await expect(sections.nth(0)).toHaveAccessibleName('core');
  await expect(sections.nth(1)).toHaveAccessibleName('myzel');
  await expect(sections.nth(2)).toHaveAccessibleName('plantarium');
  await expect(sections.nth(1).getByRole('treeitem', { name: 'cap', exact: true })).toBeVisible();
});

test('the search ignores case, Enter adds the selected node', async ({ page }) => {
  await openGraph(page);
  const search = await openMenu(page);

  await search.fill('Random');
  await expect(menu(page).getByRole('treeitem').first()).toHaveText('random');
  await expect(menu(page).getByRole('treeitem').first()).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('ArrowDown');
  await expect(menu(page).getByRole('treeitem', { name: 'random_int' })).toHaveAttribute(
    'aria-selected',
    'true'
  );
  await page.keyboard.press('Enter');

  await expect(page.getByRole('group', { name: /^random_int, node \d+$/ })).toBeVisible();
  await expect(search).toBeHidden();
});

test('the selection stays in view while the arrow keys move it', async ({ page }) => {
  await openGraph(page);
  await openMenu(page);

  for (let i = 0; i < 30; i++) await page.keyboard.press('ArrowDown');
  const selected = menu(page).locator('[aria-selected="true"]');
  await expect(selected).toBeInViewport();
});

test('dropped from a socket with Ctrl held, it lists only the nodes that fit', async ({ page }) => {
  await openGraph(page);
  const output = page.getByRole('group', { name: 'Nodes' }).getByRole('button', {
    name: 'output path of stem'
  });
  const box = await output.boundingBox();
  // the editor reacts to the keyboard and to Ctrl only once it has the focus
  await page.mouse.click(box!.x + 150, box!.y - 120);
  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2);
  await page.keyboard.down('Control');
  await page.mouse.down();
  await page.mouse.move(box!.x + 150, box!.y - 120, { steps: 8 });
  await page.mouse.up();
  await page.keyboard.up('Control');

  await expect(menu(page).getByRole('treeitem', { name: 'noise', exact: true })).toBeVisible();
  await expect(menu(page).getByRole('treeitem', { name: 'random', exact: true })).toHaveCount(0);
  await expect(menu(page).getByRole('treeitem', { name: 'stem', exact: true })).toHaveCount(0);
});
