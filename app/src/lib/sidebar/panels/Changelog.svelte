<script lang="ts">
  import { Details } from '@nodarium/ui';
  import { micromark } from 'micromark';

  type Props = {
    git?: Record<string, string>;
    changelog?: string;
  };

  const {
    git,
    changelog
  }: Props = $props();

  const typeMap = new Map([
    ['fix', 'border-l-red-800'],
    ['feat', 'border-l-green-800'],
    ['chore', 'border-l-gray-800'],
    ['docs', 'border-l-blue-800'],
    ['refactor', 'border-l-purple-800'],
    ['ci', 'border-l-red-400']
  ]);

  function detectCommitType(commit: string) {
    for (const key of typeMap.keys()) {
      if (commit.startsWith(key)) {
        return key;
      }
    }
    return '';
  }

  function parseCommit(line?: string) {
    if (!line) return;

    const regex = /^\s*-\s*\[([a-f0-9]+)\]\((https?:\/\/[^\s)]+)\)\s+(.+)$/;

    const match = line.match(regex);
    if (!match) {
      return;
    }

    const [, sha, link, description] = match;

    return {
      sha,
      link,
      description,
      type: detectCommitType(description)
    };
  }

  function parseChangelog(md: string) {
    return md.split(/^# v/gm)
      .filter(l => !!l.length)
      .map(release => {
        const [firstLine, ...rest] = release.split('\n');
        const title = firstLine.trim();

        const blocks = rest
          .join('\n')
          .split('---');

        const commits = blocks.length > 1
          ? blocks
            .at(-1)
            ?.split('\n')
            ?.map(line => parseCommit(line))
            ?.filter(c => !!c)
          : [];

        const description = (
          blocks.length > 1
            ? blocks
              .slice(0, -1)
              .join('\n')
            : blocks[0]
        ).trim();

        return {
          description: micromark(description),
          title,
          commits
        };
      });
  }
</script>

<div id="changelog" class="p-4 font-mono text-text overflow-y-auto max-h-full space-y-5">
  {#if git}
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
  {/if}

  {#if changelog}
    {#each parseChangelog(changelog) as release (release)}
      <Details title={release.title}>
        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
        <div id="description" class="pb-5">{@html release.description}</div>

        {#if release?.commits?.length}
          <Details
            title="All Commits"
            class="commits"
          >
            {#each release.commits as commit (commit)}
              <p class="py-1 leading-7 text-xs border-b-1 border-l-1 border-b-outline last:border-b-0 -ml-2 pl-2 {typeMap.get(commit.type)}">
                <!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
                <a href={commit.link} class="link" target="_blank">{commit.sha}</a>
                {commit.description}
              </p>
            {/each}
          </Details>
        {/if}
      </Details>
    {/each}
  {/if}
</div>

<style lang="postcss">
  @reference "tailwindcss";

  #changelog :global(.commits) {
    margin-left: -16px;
    margin-right: -16px;
    border-radius: 0px 0px 2px 2px !important;
  }

  #changelog :global(details > div){
    padding-bottom: 0px;
  }

  #changelog :global(.commits > div) {
    padding-bottom: 0px;
    padding-top: 0px;
  }

  #description :global(h2) {
    @apply font-bold mt-4 mb-1;
  }
  #description :global(h2:first-child) {
    margin-top: 0px !important;
  }

  #description :global(ul) {
    padding-left: 1em;
  }
  #description :global(li),
  #description :global(p) {
    @apply text-xs!;
    list-style-type: disc;
  }

  #changelog :global(details > details[open] > summary){
    margin-bottom: 20px !important;
  }

  .link {
    color: #60a5fa;
    text-decoration: none;
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
