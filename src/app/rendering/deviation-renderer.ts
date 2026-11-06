import { binDeviation, type DistributionFrame } from '../models/distribution-frame';

/**
 * Draws the deviation-from-uniformity graph: a horizontal neutral baseline
 * with 256 bins extending up (more than expected) or down (less than
 * expected). Normal sampling noise stays visually quiet near the baseline;
 * departures grow progressively taller and more saturated.
 *
 * Pure presentation: it renders the latest frame on requestAnimationFrame
 * and never influences acquisition.
 */
export class DeviationRenderer {
  private ctx: CanvasRenderingContext2D | null = null;
  private raf = 0;
  private latest: DistributionFrame | null = null;
  private drawn = false;

  /** Bins with |z| below this are treated as ordinary noise (no bar). */
  private static readonly QUIET_Z = 1.5;
  /** |z| at which the bar reaches full height and saturation. */
  private static readonly FULL_Z = 8;

  attach(canvas: HTMLCanvasElement): void {
    const dpr = window.devicePixelRatio || 1;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    canvas.width = Math.max(1, Math.round(width * dpr));
    canvas.height = Math.max(1, Math.round(height * dpr));
    this.ctx = canvas.getContext('2d');
    this.ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.drawn = false;
  }

  setFrame(frame: DistributionFrame | null): void {
    this.latest = frame;
  }

  start(): void {
    if (this.raf) return;
    const tick = () => {
      if (!this.drawn) {
        this.draw();
        this.drawn = true;
      }
      this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  }

  stop(): void {
    cancelAnimationFrame(this.raf);
    this.raf = 0;
  }

  /** Marks the buffer dirty so the next rAF redraws. */
  invalidate(): void {
    this.drawn = false;
  }

  private draw(): void {
    const ctx = this.ctx;
    if (!ctx) return;
    const canvas = ctx.canvas;
    const w = canvas.width / (window.devicePixelRatio || 1);
    const h = canvas.height / (window.devicePixelRatio || 1);

    ctx.clearRect(0, 0, w, h);

    const midY = h / 2;
    const maxBar = midY - 8;

    // Background and baseline: quiet, low contrast.
    ctx.fillStyle = '#141a20';
    ctx.fillRect(0, 0, w, h);
    ctx.strokeStyle = '#3a4752';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, midY + 0.5);
    ctx.lineTo(w, midY + 0.5);
    ctx.stroke();

    const frame = this.latest;
    if (!frame || frame.samples < 256) {
      this.drawNoData(ctx, w, h);
      return;
    }

    const binWidth = w / 256;
    for (let i = 0; i < 256; i++) {
      const z = binDeviation(frame.counts[i], frame.samples);
      const magnitude = Math.abs(z);
      if (magnitude <= DeviationRenderer.QUIET_Z) continue;

      const strength = Math.min(
        1,
        (magnitude - DeviationRenderer.QUIET_Z) /
          (DeviationRenderer.FULL_Z - DeviationRenderer.QUIET_Z),
      );
      const barHeight = Math.max(2, strength * maxBar);
      const x = Math.floor(i * binWidth);
      const widthPx = Math.max(1, Math.ceil(binWidth));

      // Color-as-state: calm gray-blue for mild deviations, warming toward
      // amber/red for strong ones. Ordinary noise never gets strong color.
      const hue = 205 - strength * 165; // 205 (blue) -> 40 (amber)
      const lightness = 45 + strength * 15;
      ctx.fillStyle = `hsl(${hue} ${30 + strength * 60}% ${lightness}%)`;

      if (z > 0) {
        ctx.fillRect(x, midY - barHeight, widthPx, barHeight);
      } else {
        ctx.fillRect(x, midY, widthPx, barHeight);
      }
    }
  }

  private drawNoData(ctx: CanvasRenderingContext2D, w: number, h: number): void {
    ctx.fillStyle = '#5f6e7a';
    ctx.font = '13px system-ui, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('collecting samples…', w / 2, h / 2 - 8);
  }
}
