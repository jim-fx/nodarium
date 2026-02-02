import * as registry from '$lib/node-registry';
import type { EntryGenerator, RequestHandler } from './$types';

export const prerender = true;

export const entries: EntryGenerator = async () => {
  const users = await registry.getUsers();
  return users
    .map((user) => {
      return user.collections.map((collection) => {
        return collection.nodes.map((node) => {
          return {
            user: user.id,
            collection: collection.id.split('/')[1],
            node: node.id.split('/')[2]
          };
        });
      });
    })
    .flat(2);
};

export const GET: RequestHandler = async function GET({ params }) {
  const wasm = await registry.getWasm(
    `${params.user}/${params.collection}/${params.node}`
  );

  if (!wasm) {
    return new Response('Not found', { status: 404 });
  }

  return new Response(wasm, {
    status: 200,
    headers: { 'Content-Type': 'application/wasm' }
  });
};
