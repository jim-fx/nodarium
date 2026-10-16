import { type Graph, GraphSchema, type ProjectFile, ProjectFileSchema } from '@nodarium/types';

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

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

// The id is reassigned on import and older graphs were saved without groups
function withGraphDefaults(graph: unknown) {
  return isObject(graph) ? { id: 0, groups: [], ...graph } : graph;
}

function parseOrThrow<T>(
  result: { success: true; data: T } | {
    success: false;
    error: { issues: { path: PropertyKey[]; message: string }[] };
  }
): T {
  if (result.success) return result.data;
  const issues = result.error.issues.map((issue) =>
    `${issue.path.map(String).join('.') || 'file'}: ${issue.message}`
  );
  throw new Error(`Invalid project file (${issues.join(', ')})`);
}

/** Accepts a project file or a bare graph (the format before project files existed) */
export function parseProjectFile(input: unknown): ProjectFile {
  if (isObject(input) && 'format' in input) {
    return parseOrThrow(
      ProjectFileSchema.safeParse({ ...input, graph: withGraphDefaults(input.graph) })
    );
  }
  const graph = parseOrThrow(GraphSchema.safeParse(withGraphDefaults(input)));
  return { format: 'nodarium-project', version: 1, graph };
}

export async function previewToBlob(preview: string) {
  const response = await fetch(preview);
  return response.blob();
}
