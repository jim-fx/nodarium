/** The app the links open, `nodarium link --base` points them at another deployment */
export const GRAPH_LINK_BASE = 'https://nodes.max-richter.dev';

/**
 * The most a link unpacks to, in bytes of JSON. The largest template is 22 KB; a few KB of
 * deflate can unpack to gigabytes, so unpacking stops here.
 */
export const MAX_GRAPH_LINK_BYTES = 1024 * 1024;

const PARAM = 'graph';

/** What is wrong with a link, in words for the person who opened it */
class LinkError extends Error {}

function toBase64Url(bytes: Uint8Array) {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function fromBase64Url(text: string) {
  if (!/^[\w-]*$/.test(text)) {
    throw new LinkError('the link has characters a graph link never has');
  }
  const binary = atob(text.replace(/-/g, '+').replace(/_/g, '/'));
  return Uint8Array.from(binary, (c) => c.charCodeAt(0));
}

const megabytes = (bytes: number) => Math.round((bytes / 1024 / 1024) * 100) / 100;

async function readAll(stream: ReadableStream<Uint8Array>, limit: number) {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.length;
    if (size > limit) {
      await reader.cancel();
      throw new LinkError(`the graph of the link is larger than ${megabytes(limit)} MB`);
    }
    chunks.push(value);
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}

/** A graph or a project file as the data of a link: its JSON, deflated and base64url */
export async function encodeGraph(graph: unknown): Promise<string> {
  const json = new Blob([JSON.stringify(graph)]);
  const deflated = json.stream().pipeThrough(new CompressionStream('deflate-raw'));
  return toBase64Url(await readAll(deflated, Infinity));
}

/**
 * The JSON in the data of a link, as `encodeGraph` wrote it. Throws an error that says what is
 * wrong with a link that is cut off, changed or unpacks to more than `limit` bytes.
 */
export async function decodeGraph(data: string, limit = MAX_GRAPH_LINK_BYTES): Promise<unknown> {
  if (data.length > limit) throw new LinkError('the link is too long');
  let bytes: Uint8Array;
  try {
    const deflated = new Blob([fromBase64Url(data) as Uint8Array<ArrayBuffer>]).stream();
    bytes = await readAll(deflated.pipeThrough(new DecompressionStream('deflate-raw')), limit);
  } catch (error) {
    if (error instanceof LinkError) throw error;
    throw new LinkError('the graph in the link is broken, it may be cut off');
  }
  try {
    return JSON.parse(new TextDecoder().decode(bytes));
  } catch {
    throw new LinkError('the link holds no JSON');
  }
}

/** The link that opens `graph` in the app at `base`, as a new project */
export async function graphLink(graph: unknown, base = GRAPH_LINK_BASE): Promise<string> {
  return `${base.replace(/\/+$/, '')}/#${PARAM}=${await encodeGraph(graph)}`;
}

/** The data of `#graph=` in a link or its fragment, undefined when it has none */
export function graphLinkData(link: string): string | undefined {
  const hash = link.indexOf('#');
  if (hash === -1) return undefined;
  return new URLSearchParams(link.slice(hash + 1)).get(PARAM) ?? undefined;
}
