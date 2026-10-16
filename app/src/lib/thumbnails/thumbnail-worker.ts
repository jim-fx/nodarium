/// <reference types="vite-plugin-comlink/client" />

export function createThumbnailWorker() {
  return new ComlinkWorker<typeof import('./thumbnail-worker-backend.ts')>(
    new URL(`./thumbnail-worker-backend.ts`, import.meta.url)
  );
}
