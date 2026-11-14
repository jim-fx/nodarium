import type { Template } from '@nodarium/runtime/templates';
import type { Graph } from '@nodarium/types';

/** Directory of the pre-rendered previews, in `app/static` and below the root of the app */
export const TEMPLATE_PREVIEW_DIR = 'template-previews';

/** The hash of each template graph that its pre-rendered preview was made from */
export type TemplatePreviewManifest = Record<string, string>;

/**
 * The url of every pre-rendered preview that still matches its template,
 * a template that changed since its preview was rendered is left out and gets built live
 */
export function selectStaticPreviews(
  manifest: TemplatePreviewManifest,
  hashes: Record<string, string>
): Record<string, string> {
  const urls: Record<string, string> = {};
  for (const [id, hash] of Object.entries(hashes)) {
    if (manifest[id] === hash) urls[id] = `/${TEMPLATE_PREVIEW_DIR}/${id}.webp?v=${hash}`;
  }
  return urls;
}

/** `null` once a preview failed, the card shows a placeholder then */
export type TemplatePreview = string | null;

/**
 * Hands every template its preview: the pre-rendered one if there is one, otherwise it is built.
 * A failed build is logged and reported as `null`, it doesn't stop the others.
 */
export function loadTemplatePreviews(
  templates: Template[],
  staticPreviews: Record<string, string>,
  render: (graph: Graph) => Promise<string | undefined>,
  onPreview: (id: string, preview: TemplatePreview) => void
): Promise<void> {
  return Promise.all(templates.map(async (template) => {
    const url = staticPreviews[template.id];
    if (url) return onPreview(template.id, url);
    try {
      onPreview(template.id, (await render(await template.load())) ?? null);
    } catch (error) {
      console.warn(`[templates] failed to render the preview of ${template.id}`, error);
      onPreview(template.id, null);
    }
  })).then(() => undefined);
}
