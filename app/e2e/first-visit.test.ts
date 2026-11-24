import { expect, type Page, test } from '@playwright/test';

test.use({ viewport: { width: 390, height: 844 }, hasTouch: true });

const APP = 'http://localhost:4173/';

test.beforeEach(({ context }) => context.route(/umami\./, (route) => route.abort()));

/** Firefox can't emulate a phone, the page is told that a finger is all there is */
async function onPhone(page: Page) {
  await page.addInitScript(() => {
    const real = window.matchMedia.bind(window);
    window.matchMedia = (query) =>
      query.includes('pointer: coarse') ? { ...real('all'), matches: true } : real(query);
  });
}

async function open(page: Page, search = '') {
  await page.goto(APP + search);
  await page.waitForFunction(() => window.nodarium, null, { polling: 100 });
  await page.evaluate(() => window.nodarium!.ready);
}

const button = (page: Page, name: string) => page.getByRole('button', { name, exact: true });

test('a phone shows the plant first: above the nodes, with the play button and no tutorial', async ({ page }) => {
  await onPhone(page);
  await open(page);
  await expect(button(page, 'Play growth')).toBeInViewport({ ratio: 1 });
  await expect(page.getByRole('dialog', { name: 'Planty' })).toBeHidden();

  const viewer = (await page.locator('[data-planty="viewer"]').boundingBox())!;
  const graph = (await page.locator('.graph-wrapper').boundingBox())!;
  expect(viewer.width).toBe(390);
  expect(graph.width).toBe(390);
  expect(graph.y).toBeGreaterThanOrEqual(viewer.y + viewer.height);
  await expect(page.locator('div.wrapper[data-node-id]').first()).toBeAttached();

  await button(page, 'Open sidebar').click();
  await expect(button(page, 'Close sidebar')).toBeInViewport({ ratio: 1 });
  await button(page, 'Close sidebar').click();
  await button(page, 'Open Planty').click();
  await expect(page.getByRole('dialog', { name: 'Planty' })).toBeVisible();
});

test('playing the growth leaves the graph as it is', async ({ page }) => {
  await open(page, '?tutorial=0');
  const state = () =>
    page.evaluate(() => {
      const { vertices, faces } = window.nodarium!.lastResult()!;
      return JSON.stringify([window.nodarium!.graph.get(), vertices, faces]);
    });
  const before = await state();

  await button(page, 'Play growth').click();
  await button(page, 'Pause growth').click();
  expect(await state()).toBe(before);
  await button(page, 'Resume growth').click();
  await expect(button(page, 'Pause growth')).toBeVisible();

  // a build of the user ends it
  await page.evaluate(() => window.nodarium!.graph.loadTemplate('fern'));
  await expect(button(page, 'Play growth')).toBeVisible();
});

test('only a graph that reads the time can be played', async ({ page }) => {
  await open(page, '?tutorial=0');
  await expect(button(page, 'Play growth')).toBeVisible();
  await page.evaluate(() =>
    window.nodarium!.graph.load({
      version: 13,
      nodes: [{ id: 1, type: 'max/plantarium/stem' }, { id: 2, type: 'max/plantarium/output' }],
      edges: [[1, 0, 2, 'input']]
    })
  );
  await expect(button(page, 'Play growth')).toBeHidden();
});
