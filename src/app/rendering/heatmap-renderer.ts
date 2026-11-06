import { binDeviation } from '../models/distribution-frame';

/**
 * Scrolling time-history heatmap: Y axis is byte value 0..255, X axis is
 * time, pixel intensity is deviation from uniformity. Newest column at the
 * right edge; older data moves left and transient disturbances leave a
 * visible trace.
 *
 * Implementation: a column of per-bin z-scores is appended per incoming
 * frame (each frame is a ~100 ms slice). The grid is drawn into an offscreen
 * canvas one column at a time and shifted left; the visible canvas is a
 * direct blit, so rendering cost is O(columns) at worst once per frame.
 */
export class HeatmapRenderer {
  private ctx: CanvasRenderingContext2D | null = null;
  private raf = 0;
  private dirty = true;

  private columns = 0;
  private rows = 256;

  /** Ring buffer of columns; each column is 256 z-scores. */
  private grid: Float32Array[] = [];

  /**
   * Experiment markers as column indices into the current grid. Indices
   * shift left with the grid so a marker stays pinned to the moment it was
   * recorded (e.g. "MAGNET_ON").
   */
  private markers: { col: number; label: string }[] = [];

  attach(canvas: HTMLCanvasElement): void {
    const dpr = window.devicePixelRatio || 1;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    canvas.width = Math.max(1, Math.round(width * dpr));
    canvas.height = Math.max(1, Math.round(height * dpr));
    this.ctx = canvas.getContext('2d');
    this.ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.columns = Math.max(1, Math.floor(width));
    this.grid = [];
    this.dirty = true;
  }

  /** Appends one time slice. Skipping slices is fine; history just advances faster. */
  push(counts: number[], samples: number): void {
    const column = new Float32Array(this.rows);
    for (let v = 0; v < this.rows; v++) {
      column[v] = samples > 0 ? binDeviation(counts[v] ?? 0, samples) : 0;
    }
    this.grid.push(column);
    if (this.grid.length > this.columns) {
      this.grid.shift();
      for (const m of this.markers) m.col -= 1;
      this.markers = this.markers.filter((m) => m.col >= 0);
    }
    this.dirty = true;
  }

  /** Pins a labeled marker at the current (rightmost) time position. */
  addMarker(label: string): void {
    this.markers.push({ col: this.grid.length - 1, label });
    this.dirty = true;
  }

  start(): void {
    if (this.raf) return;
    const tick = () => {
      if (this.dirty) {
        this.draw();
        this.dirty = false;
      }
      this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  }

  stop(): void {
    cancelAnimationFrame(this.raf);
    this.raf = 0;
  }

  private draw(): void {
    const ctx = this.ctx;
    if (!ctx) return;
    const canvas = ctx.canvas;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.width / dpr;
    const h = canvas.height / dpr;

    ctx.fillStyle = '#141a20';
    ctx.fillRect(0, 0, w, h);

    if (this.grid.length === 0) return;

    const cellW = w / this.columns;
    const cellH = h / this.rows;

    for (let c = 0; c < this.grid.length; c++) {
      const column = this.grid[c];
      const x = Math.floor(c * cellW);
      const widthPx = Math.max(1, Math.ceil(cellW));
      for (let v = 0; v < this.rows; v++) {
        const z = column[v];
        const magnitude = Math.abs(z);
        if (magnitude <= 1.5) continue;

        const strength = Math.min(1, (magnitude - 1.5) / 6.5);
        const y = Math.floor(v * cellH);
        const heightPx = Math.max(1, Math.ceil(cellH));

        // Neutral dark base -> bright trace; hue warms with strength.
        const hue = 205 - strength * 165;
        ctx.fillStyle = `hsl(${hue} ${35 + strength * 55}% ${30 + strength * 45}%)`;
        ctx.fillRect(x, y, widthPx, heightPx);
      }
    }

    // Experiment markers: vertical lines pinned to the moment they were
    // recorded, with the label at the top.
    for (const m of this.markers) {
      const x = Math.floor(m.col * cellW);
      ctx.strokeStyle = '#c9a44a';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(x + 0.5, 0);
      ctx.lineTo(x + 0.5, h);
      ctx.stroke();
      ctx.fillStyle = '#c9a44a';
      ctx.font = '10px system-ui, sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(m.label, x + 3, 10);
    }
  }
}
