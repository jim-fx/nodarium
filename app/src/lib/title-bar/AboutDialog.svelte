<script lang="ts">
  import { Button } from '@nodarium/ui';

  const PLANTARIUM_URL = 'https://github.com/jim-fx/plantarium';
  const THESIS_URL = 'https://github.com/jim-fx/bachelor';

  type Props = {
    open?: boolean;
  };

  let { open = $bindable(false) }: Props = $props();

  let dialogEl: HTMLDialogElement;

  $effect(() => {
    if (!dialogEl) return;
    if (open) {
      dialogEl.showModal();
    } else {
      dialogEl.close();
    }
  });
</script>

<dialog
  bind:this={dialogEl}
  class="m-auto max-h-[85vh] w-full max-w-lg overflow-y-auto rounded-md border border-outline bg-layer-1 p-0 text-text backdrop:bg-black/50"
  onclose={() => (open = false)}
  onclick={(e) => {
    if (e.target === dialogEl) open = false;
  }}
>
  <div class="flex flex-col gap-3 px-6 py-5">
    <img src="/max.webp" alt="Max Richter" class="mx-auto h-28 w-28 rounded-full object-cover" />
    <h3 class="m-0 text-center text-sm font-semibold">Hi, ich bin Max :)</h3>

    <div class="flex flex-col gap-3 text-xs leading-relaxed opacity-75">
      <p class="m-0">
        I'm a media designer by training, a Blender nerd and a developer. Nodarium is the thing where
        all of that ended up in one place.
      </p>
      <p class="m-0">
        It goes back to
        <a href={PLANTARIUM_URL} target="_blank" rel="noopener" class="underline">Plantarium</a>, a node based tool for generating 3D plants that I started in
        2020. I love Blender's geometry nodes, I love plants, and I wanted to make something like
        that for the browser.
      </p>
      <p class="m-0">
        Nodarium started in February 2024 as my
        <a href={THESIS_URL} target="_blank" rel="noopener" class="underline">bachelor thesis</a>. I had found WebAssembly super
        interesting, so the plan was a visual programming language where every node is its own
        WebAssembly module. That way people can write their own nodes and share them without having
        to trust each other's code. Plants stayed as the first thing to build with it.
      </p>
      <p class="m-0">
        The thesis was supposed to be three months of sweat and tears. I handed it in and then
        kept going, because I wasn't done with it.
      </p>
    </div>

    <div class="mt-1 flex justify-end">
      <Button variant="primary" onclick={() => (open = false)}>Close</Button>
    </div>
  </div>
</dialog>

<style>
  dialog {
    font-family: var(--font-family);
  }
</style>
