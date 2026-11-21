import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page, userEvent } from 'vitest/browser';
import Input from '../Input.svelte';
import InputText from './InputText.svelte';

describe('InputText', () => {
  it('shows a single line as a text field', async () => {
    render(InputText, { value: 'FFA' });

    const field = page.getByRole('textbox');
    await expect.element(field).toHaveValue('FFA');
    expect(field.element().tagName).toBe('INPUT');
  });

  it('keeps the lines of a multiline text', async () => {
    render(InputText, { value: 'X = F[+X]\nF = FF', multiline: true });

    const field = page.getByRole('textbox');
    await expect.element(field).toHaveValue('X = F[+X]\nF = FF');
    expect(field.element().tagName).toBe('TEXTAREA');
  });

  it('gives what is typed', async () => {
    const props = $state({ value: 'F' });
    render(InputText, props);

    await userEvent.type(page.getByRole('textbox'), '+G');
    expect(props.value).toBe('F+G');
  });

  it('is the field of a text input of a node', async () => {
    render(Input, {
      input: { type: 'text', multiline: true, value: 'A = F' },
      value: 'A = F[&A]'
    });

    await expect.element(page.getByRole('textbox')).toHaveValue('A = F[&A]');
  });
});
