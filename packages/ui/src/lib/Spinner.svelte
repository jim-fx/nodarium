<script lang="ts">
  interface Props {
    size?: number;
    class?: string;
  }
  let { size = 20, class: _class = '' }: Props = $props();
</script>

<svg
  class="animate-spin text-text shrink-0 {_class}"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  aria-label="Loading"
  role="status"
>
  <circle
    cx="12"
    cy="12"
    r="10"
    stroke="currentColor"
    stroke-width="2.5"
    stroke-linecap="round"
    stroke-dasharray="40 20"
  />
</svg>
