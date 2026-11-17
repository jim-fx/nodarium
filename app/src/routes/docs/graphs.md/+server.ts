import { siteDoc } from '#lib/site-files.ts';
import graphs from '../../../../../docs/GRAPHS.md?raw';
import type { RequestHandler } from './$types';

export const prerender = true;

export const GET: RequestHandler = () =>
  new Response(siteDoc(graphs), { headers: { 'Content-Type': 'text/markdown; charset=utf-8' } });
