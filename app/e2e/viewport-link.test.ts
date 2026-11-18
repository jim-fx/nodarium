import { expect, type Page, test } from '@playwright/test';

// one page for both tests, every test opens the graph it needs as a new project
test.describe.configure({ mode: 'serial' });

let page: Page;

// a thick stem straight up, the middle of the viewer is on it
const stem = {
  version: 14,
  nodes: [
    { id: 1, type: 'max/plantarium/stem', props: { length: 2, thickness: 0.6 } },
    { id: 2, type: 'max/plantarium/output' }
  ],
  edges: [[1, 0, 2, 'input']]
};

const node = (id: number) => page.locator(`.node[data-node-id="${id}"]`);
const highlighted = () => page.locator('.node.highlighted');

async function middleOfViewer() {
  const box = (await page.locator('[data-planty="viewer"] canvas').boundingBox())!;
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

/**
 * Moves the pointer onto the middle of the viewer, from a little beside it, until a node is
 * pointed out: the camera may still move to a new result
 */
async function hoverMiddle() {
  const { x, y } = await middleOfViewer();
  await expect(async () => {
    await page.mouse.move(x - 5, y);
    await page.mouse.move(x, y, { steps: 3 });
    await expect(highlighted()).toHaveCount(1, { timeout: 500 });
  }).toPass();
  return { x, y };
}

test.beforeAll(async ({ browser }) => {
  page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
  await page.goto('http://localhost:4173/?tutorial=0');
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(() => window.nodarium!.ready);
});

test.afterAll(() => page.close());

test('a part of a plant leads to its node: hover, click and the local view', async () => {
  await page.evaluate((g) => window.nodarium!.graph.load(g, { title: 'link' }), stem);
  await page.evaluate(() => window.nodarium!.idle());

  const { x, y } = await hoverMiddle();
  await expect(node(1)).toHaveClass(/highlighted/);

  await page.mouse.click(x, y);
  await expect(node(1)).toHaveClass(/active/);

  // the local view points out the chain from the node to the output
  await page.mouse.move(1, 1);
  await page.keyboard.press('i');
  await expect(node(2)).toHaveClass(/highlighted/);
  await page.keyboard.press('i');
  await expect(node(2)).not.toHaveClass(/highlighted/);

  // the pointer is outside the viewer, no part is hovered
  await expect(highlighted()).toHaveCount(0);
});

test('a tissue of a mushroom leads to the group node that made it', async () => {
  const result = await page.evaluate(() => window.nodarium!.graph.loadTemplate('fly-agaric'));
  expect(result.ok).toBe(true);
  await page.evaluate(() => window.nodarium!.idle());

  const { x, y } = await hoverMiddle();
  const id = await highlighted().getAttribute('data-node-id');

  await page.mouse.click(x, y);
  await expect(node(Number(id))).toHaveClass(/active/);
});
