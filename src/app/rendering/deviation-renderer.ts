import { binDeviation, type DistributionFrame } from '../models/distribution-frame';

/**
 * Draws the deviation-from-uniformity graph as a waterfall: every rolling
 * window (1s…5m) is one layer in depth — shortest window at the front,
 * longest at the back. Each layer is 256 bins extending up (more than
 * expected) or down (less than expected) from its own baseline.
 *
 * The window axis doubles as a persistence test: a flickering bin lights up
 * only in the short-window layers, while genuine influence forms a *ridge*
 * receding through all layers at the same byte value. The selected window
 * is drawn at full opacity; the other layers stay visible at reduced
 * opacity so persistence can be judged at a glance instead of by switching.
 *
 * Pure presentation: it renders the latest frame on requestAnimationFrame
 * and never influences acquisition.
 */
export class DeviationRenderer {
  private ctx: CanvasRenderingContext2D | null = null;
  private raf = 0;
  private latest: DistributionFrame[] = [];
  private selectedMs = 1000;
  private drawn = false;

  /** Bins with |z| below this are treated as ordinary noise (no bar). */
  private static readonly QUIET_Z = 1.5;
  /** |z| at which the bar reaches full height and saturation. */
  private static readonly FULL_Z = 8;

  private static readonly WINDOW_LABELS: Record<number, string> = {
    1000: '1s',
    5000: '5s',
    30000: '30s',
    60000: '1m',
    300000: '5m',
  };

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

  setFrames(frames: DistributionFrame[], selectedMs: number): void {
    this.latest = frames;
    this.selectedMs = selectedMs;
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
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.width / dpr;
    const h = canvas.height / dpr;

    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = '#141a20';
    ctx.fillRect(0, 0, w, h);

    const frame = this.latest;
    if (frame.length === 0) {
      this.drawNoData(ctx, w, h);
      return;
    }

    // Waterfall geometry: layer k is shifted right and up by k·depth.
    // Layer 0 (shortest window) sits at the front bottom.
    const n = frame.length;
    const depthY = Math.min(24, (h * 0.38) / n);
    const depthX = Math.min(18, w * 0.04);
    const bottom = h - 22;
    const binWidth = (w - (n - 1) * depthX) / 256;
    const upMax = bottom - (n - 1) * depthY - 6;
    const downMax = 20;
    const barWidth = Math.max(1, Math.ceil(binWidth));

    // Draw back-to-front so nearer layers overlay farther ones.
    for (let k = n - 1; k >= 0; k--) {
      const layer = frame[k];
      const xOff = k * depthX;
      const baseY = bottom - k * depthY;
      const isSelected = layer.windowMs === this.selectedMs;

      // Layer frame: baseline plus slanted rails tying the layers together.
      ctx.globalAlpha = isSelected ? 1 : 0.38;
      ctx.strokeStyle = isSelected ? '#4c5d6b' : '#333f4a';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(xOff, baseY + 0.5);
      ctx.lineTo(xOff + 256 * binWidth, baseY + 0.5);
      if (k > 0) {
        ctx.moveTo(xOff, baseY);
        ctx.lineTo(xOff - depthX, baseY + depthY);
        ctx.moveTo(xOff + 256 * binWidth, baseY);
        ctx.lineTo(xOff + 256 * binWidth - depthX, baseY + depthY);
      }
      ctx.stroke();

      // Layer label (window duration) at the left end of its baseline.
      ctx.fillStyle = isSelected ? '#8fa1b0' : '#5f6e7a';
      ctx.font = '10px system-ui, sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(
        DeviationRenderer.WINDOW_LABELS[layer.windowMs] ?? `${layer.windowMs}`,
        xOff + 2,
        baseY - 3,
      );

      if (layer.samples < 256) {
        ctx.globalAlpha = 1;
        continue;
      }

      for (let i = 0; i < 256; i++) {
        const z = binDeviation(layer.counts[i], layer.samples);
        const magnitude = Math.abs(z);
        if (magnitude <= DeviationRenderer.QUIET_Z) continue;

        const strength = Math.min(
          1,
          (magnitude - DeviationRenderer.QUIET_Z) /
            (DeviationRenderer.FULL_Z - DeviationRenderer.QUIET_Z),
        );
        const x = Math.floor(xOff + i * binWidth);

        // Color-as-state: calm gray-blue for mild deviations, warming toward
        // amber/red for strong ones. Ordinary noise never gets strong color.
        const hue = 205 - strength * 165; // 205 (blue) -> 40 (amber)
        const lightness = 45 + strength * 15;
        ctx.fillStyle = `hsl(${hue} ${30 + strength * 60}% ${lightness}%)`;

        if (z > 0) {
          ctx.fillRect(x, baseY - Math.max(2, strength * upMax), barWidth, Math.max(2, strength * upMax));
        } else {
          ctx.fillRect(x, baseY, barWidth, Math.max(2, strength * downMax));
        }
      }
      ctx.globalAlpha = 1;
    }
  }

  private drawNoData(ctx: CanvasRenderingContext2D, w: number, h: number): void {
    ctx.fillStyle = '#5f6e7a';
    ctx.font = '13px system-ui, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('collecting samples…', w / 2, h / 2 - 8);
  }
}
