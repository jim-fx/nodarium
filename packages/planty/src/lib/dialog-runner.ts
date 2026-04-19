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

  /** Walk the main path (first choice for choice nodes) and return all node IDs. */
  getMainPath(): string[] {
    const path: string[] = [];
    const visited = new Set<string>();
    let id: string | null = this.config.start;
    while (id && !visited.has(id)) {
      visited.add(id);
      path.push(id);
      const node = this.getNode(id);
      if (!node) break;
      const next = node.choices?.[0]?.next ?? node.next;
      if (next) id = next;
      else break;
    }
    return path;
  }
}
