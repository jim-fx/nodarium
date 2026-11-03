const buffer = new ArrayBuffer(4);
const view = new DataView(buffer);

export function encodeFloat(value: number): number {
  view.setFloat32(0, value, true);

  return view.getInt32(0, true);
}

export function decodeFloat(value: number): number {
  view.setInt32(0, value, true);

  return view.getFloat32(0, true);
}
