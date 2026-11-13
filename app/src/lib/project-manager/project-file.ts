import type { Graph, ProjectFile } from '@nodarium/types';

const PREVIEW_SIZE = 128;

function blobToDataUrl(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

async function createPreview(thumbnail: Blob) {
  const bitmap = await createImageBitmap(thumbnail, {
    resizeWidth: PREVIEW_SIZE,
    resizeHeight: PREVIEW_SIZE,
    resizeQuality: 'high'
  });
  const canvas = new OffscreenCanvas(PREVIEW_SIZE, PREVIEW_SIZE);
  canvas.getContext('2d')?.drawImage(bitmap, 0, 0);
  bitmap.close();
  return blobToDataUrl(await canvas.convertToBlob({ type: 'image/webp', quality: 0.8 }));
}

export async function createProjectFile(graph: Graph, thumbnail?: Blob): Promise<ProjectFile> {
  return {
    format: 'nodarium-project',
    version: 1,
    graph,
    preview: thumbnail ? await createPreview(thumbnail) : undefined
  };
}

export async function previewToBlob(preview: string) {
  const response = await fetch(preview);
  return response.blob();
}
