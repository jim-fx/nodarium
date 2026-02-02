import type { Graph } from '@nodarium/types';
import { type IDBPDatabase, openDB } from 'idb';

export interface GraphDatabase {
  projects: Graph;
}

const DB_NAME = 'nodarium-graphs';
const DB_VERSION = 1;
const STORE_NAME = 'graphs';

let dbPromise: Promise<IDBPDatabase<GraphDatabase>> | null = null;

export function getDB() {
  if (!dbPromise) {
    dbPromise = openDB<GraphDatabase>(DB_NAME, DB_VERSION, {
      upgrade(db) {
        if (!db.objectStoreNames.contains(STORE_NAME)) {
          db.createObjectStore(STORE_NAME, { keyPath: 'id' });
        }
      }
    });
  }
  return dbPromise;
}

export async function getGraph(id: number): Promise<Graph | undefined> {
  const db = await getDB();
  return db.get(STORE_NAME, id);
}

export async function saveGraph(graph: Graph): Promise<Graph> {
  const db = await getDB();
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  graph.meta = { ...graph.meta, lastModified: new Date().toISOString() };
  await db.put(STORE_NAME, graph);
  return graph;
}

export async function deleteGraph(id: number): Promise<void> {
  const db = await getDB();
  await db.delete(STORE_NAME, id);
}

export async function getGraphs(): Promise<Graph[]> {
  const db = await getDB();
  return db.getAll(STORE_NAME);
}

export async function clear(): Promise<void> {
  const db = await getDB();
  return db.clear(STORE_NAME);
}
