import type { WindowStats } from './stream-status';

/**
 * The frame a renderer draws. Renderers keep only the latest frame; skipped
 * intermediate frames are expected and harmless (Rule 5).
 */
export interface DistributionFrame {
  windowMs: number;
  samples: number;
  counts: number[];
}

export function toFrame(stats: WindowStats | undefined): DistributionFrame | null {
  if (!stats || !stats.counts || stats.counts.length !== 256) {
    return null;
  }
  return {
    windowMs: stats.windowMs,
    samples: stats.samples,
    counts: stats.counts,
  };
}

/**
 * Deviation of one bin from uniformity, in approximate standard deviations:
 * z = (count - expected) / sqrt(expected). Ordinary sampling noise for a
 * uniform source sits around |z| <= ~2; larger |z| indicates real bias.
 */
export function binDeviation(count: number, samples: number): number {
  if (samples <= 0) return 0;
  const expected = samples / 256;
  if (expected <= 0) return 0;
  return (count - expected) / Math.sqrt(expected);
}
