import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';
import ShortCut from './ShortCut.svelte';

describe('ShortCut', () => {
  it('should render with key label', async () => {
    render(ShortCut, { key: 'S' });

    const shortcut = page.getByText('S');
    await expect.element(shortcut).toBeInTheDocument();
  });

  it('should render ctrl modifier', async () => {
    render(ShortCut, { ctrl: true, key: 'S' });

    const shortcut = page.getByText(/Ctrl/);
    await expect.element(shortcut).toBeInTheDocument();
  });

  it('should render alt modifier', async () => {
    render(ShortCut, { alt: true, key: 'F4' });

    const shortcut = page.getByText(/Alt/);
    await expect.element(shortcut).toBeInTheDocument();
  });

  it('should render multiple modifiers', async () => {
    render(ShortCut, { ctrl: true, alt: true, key: 'Delete' });

    const shortcut = page.getByText(/Ctrl/);
    await expect.element(shortcut).toBeInTheDocument();
  });
});
