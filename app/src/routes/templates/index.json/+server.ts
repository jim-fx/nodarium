import { templateIndex } from '#lib/site-files.ts';
import type { RequestHandler } from './$types';

export const prerender = true;

export const GET: RequestHandler = async () => Response.json(await templateIndex());
