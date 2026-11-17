/** What the address of the page asks for, read once when the page loads */
export type UrlOptions = {
  /** `?tutorial=0` keeps Planty from starting on its own for this visit */
  tutorial: boolean;
};

export function readUrlOptions(search: string): UrlOptions {
  const params = new URLSearchParams(search);
  return { tutorial: params.get('tutorial') !== '0' };
}
