export const prerender = true;

export async function load({ fetch }) {
  async function fetchChangelog() {
    try {
      const res = await fetch('/CHANGELOG.md');
      return await res.text();
    } catch (error) {
      console.log('Failed to fetch CHANGELOG.md', error);
      return;
    }
  }

  async function fetchGitInfo() {
    try {
      const res = await fetch('/git.json');
      return await res.json();
    } catch (error) {
      console.log('Failed to fetch git.json', error);
      return;
    }
  }

  return {
    git: await fetchGitInfo(),
    changelog: await fetchChangelog()
  };
}
