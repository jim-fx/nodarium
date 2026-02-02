// Create a buffer to hold the float as bytes
const buffer = new ArrayBuffer(4);
const view = new DataView(buffer);

export function encodeFloat(value: number): number {
  // Write the number as a float to the buffer
  view.setFloat32(0, value, true); // 'true' for little-endian

  // Read the buffer as an integer
  return view.getInt32(0, true);
}

export function decodeFloat(value: number): number {
  // Write the integer back as an int32
  view.setInt32(0, value, true);

  // Read the buffer as a float
  return view.getFloat32(0, true);
}
