<script lang="ts">
  interface Props {
    value: boolean;
    id?: string;
  }

  let { value = $bindable(false), id }: Props = $props();

  $effect(() => {
    if (typeof value === 'string') {
      value = value === 'true';
    } else if (typeof value === 'number') {
      value = value === 1;
    } else if (!(typeof value === 'boolean')) {
      value = !!value;
    }
  });
</script>

<label
  class="relative inline-flex h-5.5 w-5.5 cursor-pointer items-center justify-center bg-layer-2 rounded-[5px]"
>
  <input
    type="checkbox"
    bind:checked={value}
    class="peer absolute h-px w-px overflow-hidden whitespace-nowrap border-0 p-0 [clip:rect(0,0,0,0)]"
    {id}
  />
  <span
    class="absolute opacity-0 peer-checked:opacity-100 transition-opacity duration-100 flex w-full h-full items-center justify-center"
  >
    <svg
      viewBox="0 0 19 14"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      class="h-2.5 w-3 text-text"
    >
      <path
        d="M2 7L7 12L17 2"
        stroke="currentColor"
        stroke-width="3"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  </span>
</label>
