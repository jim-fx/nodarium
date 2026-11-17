import { templates } from '@nodarium/runtime/templates';
import { error } from '@sveltejs/kit';
import type { EntryGenerator, RequestHandler } from './$types';

export const prerender = true;

export const entries: EntryGenerator = () => templates.map((template) => ({ id: template.id }));

export const GET: RequestHandler = async ({ params }) => {
  const template = templates.find((t) => t.id === params.id);
  if (!template) error(404, `There is no template "${params.id}"`);
  return Response.json(await template.load());
};
