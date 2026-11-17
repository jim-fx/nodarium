import { graphFileJsonSchema } from '@nodarium/runtime';
import type { RequestHandler } from './$types';

export const prerender = true;

export const GET: RequestHandler = () => Response.json(graphFileJsonSchema());
