import type { RequestHandler } from './$types';

import * as registry from '#lib/node-registry.ts';

export const prerender = true;

export const GET: RequestHandler = async function GET() {
  const users = await registry.getUsers();

  return Response.json(users);
};
