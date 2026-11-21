import { expect, test } from '@playwright/test';

test.use({ viewport: { width: 1600, height: 900 } });

const APP = process.env.APP_URL ?? 'http://localhost:4173/';

test('/template changes a value and a material, and opens the changed template in the editor', async ({ page, context }) => {
  await page.goto(`${APP}template?template=default-plant`);

  const stats = page.getByRole('status').filter({ hasText: 'vertices' });
  await expect(stats).not.toContainText(/^\s*0 vertices/);
  await expect(page.getByRole('tab', { name: 'Plants' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('button', { name: 'Porcini' })).toHaveCount(0);

  const nodes = page.getByRole('button', { name: 'Nodes', exact: true });
  await expect(nodes).toHaveAttribute('aria-pressed', 'false');
  await nodes.click();
  // the leaves on the instance node: 8 to 14 in the template, 20 to 30 after
  const leaves = (input: string) =>
    page.locator(`[data-node-id="56"] [data-node-input="${input}"] input`);
  await expect(leaves('max')).toBeAttached();
  const before = await stats.textContent();

  // the camera shows all nodes, too far away for their inputs to be drawn
  for (const [input, value] of [['max', '30'], ['min', '20']]) {
    await leaves(input).evaluate((field: HTMLInputElement, text) => {
      field.value = text;
      field.dispatchEvent(new Event('change', { bubbles: true }));
    }, value);
  }
  await expect(stats).not.toHaveText(before!);
  await expect(page.getByRole('button', { name: 'Reset' })).toBeEnabled();

  // the edit stays when the nodes are hidden and shown again
  await nodes.click();
  await nodes.click();
  await expect(leaves('max')).toHaveValue('30');

  const groups = page.getByRole('radiogroup', { name: 'Material' }).getByRole('radio', {
    name: 'Groups'
  });
  await groups.click();
  await expect(groups).toHaveAttribute('aria-checked', 'true');

  const [editor] = await Promise.all([
    context.waitForEvent('page'),
    page.getByRole('button', { name: 'Open in editor' }).click()
  ]);
  await editor.waitForFunction(() => window.nodarium, null, { polling: 100 });
  const opened = await editor.evaluate(async () => {
    await window.nodarium!.ready;
    await window.nodarium!.idle();
    return window.nodarium!.graph.get();
  });
  expect(opened.meta?.title).toBe('Default Plant');
  expect(opened.nodes.find((n) => n.id === 56)?.props).toEqual({ min: 20, max: 30 });
});

test('/template lists the mushrooms on their own tab, scrubs the time and keeps the material', async ({ page }) => {
  await page.goto(`${APP}template?template=porcini`);

  await expect(page.getByRole('tab', { name: 'Mushrooms' })).toHaveAttribute(
    'aria-selected',
    'true'
  );
  await expect(page.getByRole('button', { name: 'Porcini' })).toHaveAttribute(
    'aria-current',
    'page'
  );
  await expect(page.getByRole('button', { name: 'Fern', exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Nodes', exact: true })).toHaveAttribute(
    'aria-pressed',
    'false'
  );

  const stats = page.getByRole('status').filter({ hasText: 'vertices' });
  await expect(stats).not.toContainText(/^\s*0 vertices/);
  const grown = await stats.textContent();

  await page.getByRole('slider', { name: 'Time' }).fill('0.2');
  await expect(page.locator('output')).toHaveText('0.20');
  await expect(stats).not.toHaveText(grown!);

  // the material is remembered for the next visit
  const groups = page.getByRole('radiogroup', { name: 'Material' }).getByRole('radio', {
    name: 'Groups'
  });
  await groups.click();
  await page.reload();
  await expect(groups).toHaveAttribute('aria-checked', 'true');

  await page.getByRole('tab', { name: 'Plants' }).click();
  await page.getByRole('button', { name: 'Fern', exact: true }).click();
  await expect(page).toHaveURL(/template=fern/);
});
