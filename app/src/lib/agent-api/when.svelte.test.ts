import { describe, expect, it } from 'vitest';
import { when } from './when.svelte';

// effects only run with the client build of svelte, `vitest --browser=false` uses the server one
describe.skipIf(import.meta.env.SSR)('when', () => {
  it('resolves at once when the condition holds', async () => {
    await expect(when(() => true)).resolves.toBeUndefined();
  });

  it('resolves once the state it reads makes the condition true', async () => {
    const state = $state({ running: 2 });
    let resolved = false;
    const done = when(() => state.running === 0).then(() => (resolved = true));

    state.running = 1;
    await new Promise((r) => setTimeout(r));
    expect(resolved).toBe(false);

    state.running = 0;
    await done;
    expect(resolved).toBe(true);
  });
});
