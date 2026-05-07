export type ToastType = 'info' | 'success' | 'error';

export type ToastItem = {
  id: number;
  message: string;
  type: ToastType;
};

let _toasts = $state<ToastItem[]>([]);
let _nextId = 0;

export const toasts = {
  get value() {
    return _toasts;
  }
};

export function toast(message: string, type: ToastType = 'info', duration = 3000) {
  const id = _nextId++;
  _toasts.push({ id, message, type });
  setTimeout(() => {
    _toasts = _toasts.filter((t) => t.id !== id);
  }, duration);
}
