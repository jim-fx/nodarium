import type { Choice, DialogNode, PlantyConfig } from './types.js';

export class DialogRunner {
  private config: PlantyConfig;

  constructor(config: PlantyConfig) {
    this.config = config;
  }

  getNode(id: string): DialogNode | null {
    return this.config.nodes[id] ?? null;
  }

  getStartNode(): { id: string; node: DialogNode } | null {
    const node = this.getNode(this.config.start);
    if (!node) return null;
    return { id: this.config.start, node };
  }

  getNextNode(currentId: string): { id: string; node: DialogNode } | null {
    const current = this.getNode(currentId);
    if (!current) return null;
    if (!current.next) return null;
    const next = this.getNode(current.next);
    if (!next) return null;
    return { id: current.next, node: next };
  }

  followChoice(choice: Choice): { id: string; node: DialogNode } | null {
    if (!choice.next) return null;
    const node = this.getNode(choice.next);
    if (!node) return null;
    return { id: choice.next, node };
  }

  /** Follow the `next` links from `id` and return all node IDs, including `id`. */
  getChain(id: string): string[] {
    const chain: string[] = [];
    const visited = new Set<string>();
    let current: string | null | undefined = id;
    while (current && !visited.has(current) && this.getNode(current)) {
      visited.add(current);
      chain.push(current);
      current = this.getNode(current)?.next;
    }
    return chain;
  }

  /** Return a list of problems: missing start, dangling links and unreachable nodes. */
  validate(): string[] {
    const problems: string[] = [];
    const { start, nodes } = this.config;
    if (!nodes[start]) problems.push(`start node "${start}" does not exist`);

    for (const [id, node] of Object.entries(nodes)) {
      if (node.next && !nodes[node.next]) {
        problems.push(`"${id}" links to missing node "${node.next}"`);
      }
      for (const choice of node.choices ?? []) {
        if (choice.next && !nodes[choice.next]) {
          problems.push(
            `choice "${choice.label}" of "${id}" links to missing node "${choice.next}"`
          );
        }
      }
    }

    const reachable = new Set<string>();
    const queue = nodes[start] ? [start] : [];
    while (queue.length) {
      const id = queue.pop()!;
      if (reachable.has(id)) continue;
      reachable.add(id);
      const node = nodes[id];
      const links = [node.next, ...(node.choices ?? []).map((c) => c.next)];
      for (const link of links) {
        if (link && nodes[link]) queue.push(link);
      }
    }
    for (const id of Object.keys(nodes)) {
      if (!reachable.has(id)) problems.push(`"${id}" is unreachable`);
    }

    return problems;
  }
}
