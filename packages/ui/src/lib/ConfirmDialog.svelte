<script lang="ts">
  import type { Snippet } from 'svelte';
  import Button from './Button.svelte';

  interface Props {
    open?: boolean;
    title?: string;
    message?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    onconfirm?: () => void;
    oncancel?: () => void;
    children?: Snippet;
  }

  let {
    open = $bindable(false),
    title = 'Are you sure?',
    message,
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    onconfirm,
    oncancel,
    children
  }: Props = $props();

  let dialogEl: HTMLDialogElement;

  $effect(() => {
    if (!dialogEl) return;
    if (open) {
      dialogEl.showModal();
    } else {
      dialogEl.close();
    }
  });

  function confirm() {
    open = false;
    onconfirm?.();
  }

  function cancel() {
    open = false;
    oncancel?.();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      confirm();
    }
  }

  function handleCancel(e: Event) {
    e.preventDefault();
    cancel();
  }
</script>

<dialog
  bind:this={dialogEl}
  class="m-auto bg-layer-1 border border-outline rounded-md p-0 text-text max-w-md w-full backdrop:bg-black/50"
  oncancel={handleCancel}
  onkeydown={handleKeydown}
  onclick={(e) => {
    if (e.target === dialogEl) cancel();
  }}
>
  <div class="px-6 py-5 flex flex-col gap-3">
    <h3 class="m-0 text-sm font-semibold">{title}</h3>
    {#if message}
      <p class="m-0 text-xs opacity-75 leading-relaxed">{message}</p>
    {/if}
    {#if children}
      <div class="text-xs">
        {@render children()}
      </div>
    {/if}
    <div class="flex justify-end gap-2 mt-1">
      <Button onclick={cancel}>{cancelLabel}</Button>
      <Button variant="primary" onclick={confirm}>{confirmLabel}</Button>
    </div>
  </div>
</dialog>

<style>
  dialog {
    font-family: var(--font-family);
  }

  dialog::backdrop {
    background: rgba(0, 0, 0, 0.5);
  }
</style>
