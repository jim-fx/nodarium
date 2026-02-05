<script lang="ts">
  type Change = { type: string; content: string };

  async function fetchChangelog() {
    const res = await fetch('/CHANGELOG.md');
    return await res.text();
  }

  const typeMap: Record<string, string> = {
    fix: 'bg-layer-2 bg-red-800',
    feat: 'bg-layer-2 bg-green-800',
    chore: 'bg-layer-2 bg-gray-800',
    docs: 'bg-layer-2 bg-blue-800',
    refactor: 'bg-layer-2 bg-purple-800',
    default: 'bg-layer-2 text-text'
  };

  function parseChangelog(md: string) {
    const lines = md.split('\n');
    const parsed: (string | Change)[] = [];

    for (let line of lines) {
      line = line.trim();
      if (!line) continue;

      if (line === '---') {
        parsed.push({ type: 'hr', content: '' });
        continue;
      }

      // Headers
      if (line.startsWith('## ')) {
        parsed.push(line.replace('## ', ''));
        continue;
      }

      // Commit type
      const match = line.match(/^(fix|feat|chore|docs|refactor)(\(|:)/i);
      if (match) {
        parsed.push({ type: match[1].toLowerCase(), content: line });
        continue;
      }

      // Other lines
      parsed.push({ type: 'default', content: line });
    }

    let lastLine = parsed.at(-1);
    if (lastLine !== undefined && typeof lastLine !== 'string' && lastLine.type === 'hr') {
      parsed.pop();
    }

    return parsed;
  }
</script>

<div class="p-4 font-mono text-text">
  {#await fetchChangelog()}
    <p>Loading...</p>
  {:then md}
    {#each parseChangelog(md) as item (item)}
      {#if typeof item === 'string'}
        <h2 class="text-xl font-semibold mt-4 mb-4 text-layer-1">{item}</h2>
      {:else if item.type === 'hr'}
        <p></p>
      {:else}
        <p class="px-3 py-1 mb-1 leading-8 border-b-1 border-b-outline last:border-b-0">
          {#if item.type !== 'default'}
            <span class="p-1 rounded-sm font-semibold {typeMap[item.type]}">
              {item.content.split(':')[0]}
            </span> {item.content.split(':').slice(1).join(':').trim()}
          {:else}
            {item.content}
          {/if}
        </p>
      {/if}
    {/each}
  {/await}
</div>
