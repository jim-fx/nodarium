<script lang="ts">
  interface Props {
    value: boolean;
    id?: string;
  }

  let { value = $bindable(false), id }: Props = $props();

  // node props hold 1 and 0, the value only becomes a boolean when the user changes it
  function isChecked(v: unknown) {
    if (typeof v === 'string') return v === 'true';
    if (typeof v === 'number') return v === 1;
    return !!v;
  }
</script>

<label
  class="relative inline-flex h-5.5 w-5.5 cursor-pointer items-center justify-center bg-layer-2 outline-1 outline-outline rounded-[5px]"
>
  <input
    type="checkbox"
    bind:checked={() => isChecked(value), (checked) => (value = checked)}
    class="peer absolute h-px w-px overflow-hidden whitespace-nowrap border-0 p-0 [clip:rect(0,0,0,0)]"
    {id}
  />
  <span
    class="absolute opacity-0 peer-checked:opacity-100 transition-opacity duration-50 flex w-full h-full items-center justify-center"
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
