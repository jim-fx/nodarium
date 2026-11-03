<script lang="ts">
  import { Details } from '@nodarium/ui';
  import { micromark } from 'micromark';
  import { groupByMilestone, type RoadmapItem, type RoadmapKind } from './roadmap';

  const { items }: { items: RoadmapItem[] } = $props();

  const sections: { kind: RoadmapKind; title: string; accent: string }[] = [
    { kind: 'feat', title: 'Planned', accent: 'border-l-green-800' },
    { kind: 'concept', title: 'Concepts', accent: 'border-l-gray-600 opacity-75' }
  ];
</script>

{#if items.length === 0}
  <p class="text-xs opacity-50">The roadmap could not be loaded.</p>
{/if}

{#each sections as section (section.kind)}
  {@const groups = groupByMilestone(items.filter((i) => i.kind === section.kind))}
  {#if groups.length}
    <Details title={section.title} open={section.kind === 'feat'}>
      {#each groups as group (group.milestone)}
        {#if group.milestone}
          <h3 class="mt-4 mb-2 text-xs font-bold first:mt-0">{group.milestone}</h3>
        {/if}
        {#each group.items as item (item.number)}
          <div class="py-2 pl-2 -ml-2 border-l-1 border-b-1 border-b-outline last:border-b-0 text-xs leading-5 {section.accent}">
            <!-- eslint-disable svelte/no-navigation-without-resolve -->
            <a
              href={item.html_url}
              target="_blank"
              class="text-blue-400! no-underline hover:underline"
            >
              #{item.number}
            </a>
            <!-- eslint-enable svelte/no-navigation-without-resolve -->
            {item.title}
            {#if item.summary}
              <!-- eslint-disable-next-line svelte/no-at-html-tags -->
              <div class="mt-1 opacity-60 [&_p]:m-0">{@html micromark(item.summary)}</div>
            {/if}
          </div>
        {/each}
      {/each}
    </Details>
  {/if}
{/each}
