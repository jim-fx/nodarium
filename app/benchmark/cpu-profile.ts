// What `node --cpu-prof` and the inspector's Profiler write, read for pnpm profile:node

export type CpuProfile = {
  nodes: { id: number; callFrame: { functionName: string; url: string } }[];
  samples: number[];
  timeDeltas: number[];
};

/**
 * The path of a Rust symbol in the v0 mangling, `myzel_core::density::ClosedSearch::block` of
 * `_RNvMs0_NtCs…_10myzel_core7densityNtB5_12ClosedSearch5block`. Only its identifiers, in order:
 * generic arguments come out as more path segments, which is enough to know the function.
 */
export function demangle(symbol: string) {
  if (!symbol.startsWith('_R')) return symbol;
  const parts: string[] = [];
  let i = 2;
  while (i < symbol.length) {
    const c = symbol[i];
    if (c >= '0' && c <= '9') {
      let digits = '';
      while (symbol[i] >= '0' && symbol[i] <= '9') digits += symbol[i++];
      if (symbol[i] === '_') i++;
      const length = Number(digits);
      if (length) parts.push(symbol.slice(i, i + length));
      i += length;
    } else if (c === 'B' || c === 's') {
      // a back reference or a disambiguator: base-62 digits up to `_`
      const end = symbol.indexOf('_', i);
      i = end < 0 ? symbol.length : end + 1;
    } else {
      i++;
    }
  }
  return parts.length ? parts.join('::') : symbol;
}

/** The functions with the most time of their own, in ms */
export function topFunctions(profile: CpuProfile, count: number) {
  const frames = new Map(profile.nodes.map(node => [node.id, node.callFrame]));
  const self = new Map<string, number>();
  profile.samples.forEach((sample, i) => {
    const frame = frames.get(sample)!;
    const name = frame.url.startsWith('wasm://')
      ? demangle(frame.functionName)
      : frame.url
      ? `${frame.functionName || '(anonymous)'} (js)`
      : frame.functionName;
    self.set(name, (self.get(name) ?? 0) + (profile.timeDeltas[i] ?? 0) / 1000);
  });
  const total = [...self.values()].reduce((a, b) => a + b, 0);
  const top = [...self].sort((a, b) => b[1] - a[1]).slice(0, count);
  return { total, top: top.map(([name, ms]) => ({ name, ms, share: total ? ms / total : 0 })) };
}
