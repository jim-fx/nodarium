import { nodeCatalog } from '#lib/site-files.ts';
import type { RequestHandler } from './$types';

export const prerender = true;

export const GET: RequestHandler = async () => {
  const { text } = await nodeCatalog();
  return new Response(text, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
};
