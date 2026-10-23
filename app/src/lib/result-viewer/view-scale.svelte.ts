/** The size of the content relative to a plant that is about 8 meters across, gizmos scale with it */
export const viewScale = $state({ value: 1 });

export function updateViewScale(radius: number) {
  viewScale.value = radius / 4;
}
