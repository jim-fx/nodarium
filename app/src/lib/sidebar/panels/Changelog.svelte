<script lang="ts">
  import { Details } from '@nodarium/ui';

  type ReleaseBlock = {
    header: string;
    sections: { title: string; items: { type: string; text?: string; url?: string; linkText?: string; linkUrl?: string; message?: string; content?: string }[] }[];
    commits: { type: string; linkText: string; linkUrl: string; message: string }[];
  };

  const typeMap: Record<string, string> = {
    fix: 'bg-red-800',
    feat: 'bg-green-800',
    chore: 'bg-gray-800',
    docs: 'bg-blue-800',
    refactor: 'bg-purple-800',
    default: ''
  };

  const sectionHeaders = ['Features', 'Fixes', 'Maintenance / CI', 'Maintenance'];

  async function fetchChangelog() {
    const res = await fetch('/CHANGELOG.md');
    return await res.text();
  }

  async function fetchGitInfo() {
    const res = await fetch('/git.json');
    return await res.json();
  }

  function parseChangelog(md: string): ReleaseBlock[] {
    const lines = md.split('\n');
    const releases: ReleaseBlock[] = [];
    let currentRelease: ReleaseBlock | null = null;

    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (!line) continue;

      if (line === '---') {
        currentRelease = null;
        continue;
      }

      if (line.startsWith('## ')) {
        currentRelease = {
          header: line.replace('## ', ''),
          sections: [],
          commits: []
        };
        releases.push(currentRelease);
        continue;
      }

      if (!currentRelease) continue;

      if (line.startsWith('### ')) {
        currentRelease.commits = [];
        continue;
      }

      const commitMatch = line.match(/^- \[([^\]]+)\]\(([^)]+)\)(.+)$/);
      if (commitMatch) {
        currentRelease.commits.push({
          type: 'commit',
          linkText: commitMatch[1],
          linkUrl: commitMatch[2],
          message: commitMatch[3].trim()
        });
        continue;
      }

      if (sectionHeaders.includes(line)) {
        currentRelease.sections.push({ title: line, items: [] });
        continue;
      }

      const lastSection = currentRelease.sections.at(-1);
      if (lastSection) {
        const match = line.match(/^(fix|feat|chore|docs|refactor)(\(|:)/i);
        if (match) {
          lastSection.items.push({ type: match[1].toLowerCase(), content: line });
        } else {
          lastSection.items.push({ type: 'default', content: line });
        }
      }
    }

    return releases;
  }
</script>

<div class="p-4 font-mono text-text overflow-y-auto max-h-full">
  {#await Promise.all([fetchChangelog(), fetchGitInfo()])}
    <p>Loading...</p>
  {:then [md, git]}
    <div class="mb-4 p-3 bg-layer-2 text-xs rounded">
      <p><strong>Branch:</strong> {git.branch}</p>
      <p>
        <strong>Commit:</strong>
        <a
          href="https://git.max-richter.dev/max/nodarium/commit/{git.sha}"
          class="link"
          target="_blank"
        >
          {git.sha.slice(0, 7)}
        </a>
        – {git.commit_message}
      </p>
      <p>
        <strong>Commits since last release:</strong>
        {git.commits_since_last_release}
      </p>
      <p>
        <strong>Timestamp:</strong>
        {new Date(git.commit_timestamp).toLocaleString()}
      </p>
    </div>

    {#each parseChangelog(md) as release}
      <hr class="border-outline my-4" />
      <h2 class="text-xl font-semibold mt-4 mb-3 text-layer-1">{release.header}</h2>

      {#each release.sections as section}
        <h3 class="text-base font-semibold mt-3 mb-2 text-layer-2">{section.title}</h3>
        {#each section.items as item}
          {#if item.type === 'default'}
            <p class="py-1 leading-7">{item.content}</p>
          {:else}
            <p class="py-1 leading-7">
              <span
                class="p-1 rounded-sm opacity-80 font-semibold {typeMap[item.type] ?? 'bg-layer-2'}"
              >
                {item.content?.split(':')[0]}
              </span>
              {item.content?.split(':').slice(1).join(':').trim()}
            </p>
          {/if}
        {/each}
      {/each}

      {#if release.commits.length > 0}
        <Details title="All Commits" transparent>
          {#each release.commits as item}
            <p class="py-1 leading-7">
              <a href={item.linkUrl} class="link" target="_blank">{item.linkText}</a>
              {' '}{item.message}
            </p>
          {/each}
        </Details>
      {/if}
    {/each}
  {/await}
</div>

<style>
  .link {
    color: #60a5fa;
    text-decoration: none;
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
