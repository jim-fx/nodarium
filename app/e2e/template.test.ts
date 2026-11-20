import { expect, test } from '@playwright/test';

test.use({ viewport: { width: 1600, height: 900 } });

const APP = process.env.APP_URL ?? 'http://localhost:4173/';

test('/template changes a value and a material, and opens the changed template in the editor', async ({ page, context }) => {
  await page.goto(`${APP}template?template=default-plant`);

  const stats = page.getByRole('status').filter({ hasText: 'vertices' });
  await expect(stats).not.toContainText(/^\s*0 vertices/);
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

  const material = page.getByLabel('Material');
  await material.selectOption('Groups');
  await expect(material).toHaveValue(
    await material.locator('option', { hasText: 'Groups' }).getAttribute('value') ?? ''
  );

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
