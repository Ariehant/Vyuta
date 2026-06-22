// Loader for the probe-rs Neon addon (rust/probe-rs-extension).
//
// The addon is an in-process native module providing synchronous probe/target
// queries. It is loaded lazily and degrades gracefully if it hasn't been built
// (`npm run build` in rust/probe-rs-extension), so the rest of the extension
// keeps working.

import * as path from "path";

export interface ProbeInfo {
  identifier: string;
  vendorId: number;
  productId: number;
  serialNumber: string | null;
}

export interface ProbeAddon {
  hello(): string;
  listProbes(): ProbeInfo[];
  listChipFamilies(): string[];
}

let cached: ProbeAddon | null | undefined;

export function loadProbeAddon(): ProbeAddon | null {
  if (cached !== undefined) {
    return cached;
  }
  const candidates = [
    // Monorepo dev layout: extensions/drone-firmware/out -> rust/probe-rs-extension.
    path.join(__dirname, "..", "..", "..", "rust", "probe-rs-extension", "index.cjs"),
    // Packaged layout: addon bundled alongside the extension.
    path.join(__dirname, "..", "probe-rs-extension", "index.cjs"),
  ];
  for (const candidate of candidates) {
    try {
      // eslint-disable-next-line @typescript-eslint/no-var-requires
      cached = require(candidate) as ProbeAddon;
      return cached;
    } catch {
      /* try the next candidate */
    }
  }
  cached = null;
  return cached;
}
