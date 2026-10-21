import { untrack } from 'svelte';
import type { Attachment } from 'svelte/attachments';

/**
 * Keeps a <dialog> in sync with `open`. Use as `{@attach modal(open)}`,
 * `onshow` runs right after the dialog was opened.
 */
export function modal(open: boolean, onshow?: () => void): Attachment<HTMLDialogElement> {
  return (dialog) => {
    if (open && !dialog.open) {
      dialog.showModal();
      if (onshow) untrack(onshow);
    } else if (!open && dialog.open) {
      dialog.close();
    }
  };
}
