<script lang="ts">
  import type { GraphManager } from '$lib/graph-interface/graph-manager.svelte';
  import type { GroupDefinition } from '@nodarium/types';
  import { SvelteMap, SvelteSet } from 'svelte/reactivity';

  type Props = { manager: GraphManager };
  const { manager }: Props = $props();

  type GroupNode = { group: GroupDefinition; children: GroupNode[] };

  const unusedTree = $derived.by((): GroupNode[] => {
    const unused = manager.getUnusedGroups();
    if (!unused.length) return [];

    const unusedIds = new Set(unused.map(g => g.id));

    // Build child map: which unused groups reference which other unused groups
    const childrenOf = new SvelteMap<number, number[]>();
    const referencedBy = new SvelteSet<number>();

    for (const group of unused) {
      const refs: number[] = [];
      for (const node of group.nodes) {
        if (node.type === '__internal/group/instance' && node.props?.groupId !== undefined) {
          const childId = node.props.groupId as number;
          if (unusedIds.has(childId)) {
            refs.push(childId);
            referencedBy.add(childId);
          }
        }
      }
      childrenOf.set(group.id, refs);
    }

    const byId = new Map(unused.map(g => [g.id, g]));

    function buildNode(g: GroupDefinition): GroupNode {
      return {
        group: g,
        children: (childrenOf.get(g.id) ?? []).map(id => buildNode(byId.get(id)!))
      };
    }

    return unused
      .filter(g => !referencedBy.has(g.id))
      .map(buildNode);
  });
</script>

{#if unusedTree.length}
  <div class="panel p-4">
    <div class="header">
      <span>Unused groups</span>
      <button class="remove-all" onclick={() => manager.removeUnusedGroups()}>
        Remove all
      </button>
    </div>

    <ul class="tree">
      {#snippet treeNode(node: GroupNode)}
        <li>
          <span class="group-name">{node.group.name || `Group #${node.group.id}`}</span>
          {#if node.children.length}
            <ul>
              {#each node.children as child (child.group.id)}
                {@render treeNode(child)}
              {/each}
            </ul>
          {/if}
        </li>
      {/snippet}
      {#each unusedTree as node (node.group.id)}
        {@render treeNode(node)}
      {/each}
    </ul>
  </div>
{/if}

<style>
  .panel {
    border-top: 1px solid var(--color-outline);
    margin-top: -1px;
    border-bottom: 1px solid var(--color-outline);
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.5em;
    font-size: 0.8em;
    opacity: 0.7;
  }

  .remove-all {
    background: none;
    border: 1px solid var(--color-outline);
    border-radius: 4px;
    color: var(--color-text);
    cursor: pointer;
    font-family: var(--font-family);
    font-size: 0.85em;
    padding: 0.2em 0.5em;
  }

  .remove-all:hover {
    border-color: var(--color-active);
  }

  .tree {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .tree ul {
    list-style: none;
    margin: 0;
    padding-left: 1.2em;
    border-left: 1px solid var(--color-outline);
  }

  .tree li {
    padding: 0.15em 0;
  }

  .group-name {
    font-size: 0.85em;
  }

  .tree ul .group-name::before {
    content: '└ ';
    opacity: 0.4;
  }
</style>
