import * as registry from '$lib/node-registry';
import { json } from '@sveltejs/kit';
import type { EntryGenerator, RequestHandler } from './$types';

export const prerender = true;

export const entries: EntryGenerator = async () => {
  const users = await registry.getUsers();
  return users.map(user => {
    return { user: user.id };
  }).flat(2);
};

export const GET: RequestHandler = async function GET({ params }) {
  const namespaces = await registry.getUser(params.user);

  return json(namespaces);
};
