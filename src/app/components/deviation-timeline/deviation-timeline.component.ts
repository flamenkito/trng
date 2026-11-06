import {
  Component,
  ElementRef,
  OnDestroy,
  OnInit,
  effect,
  input,
  viewChild,
} from '@angular/core';

/**
 * One-pixel-per-snapshot sparkline of max |z| across bins. External
 * influence shows up here as a *rise* (onset), a *plateau* (duration), and
 * a *fall* (recovery) — the clearest single signal for interventions.
 * Horizontal reference lines mark |z| = 2 (noise ceiling) and |z| = 4
 * (alert threshold).
 */
@Component({
  selector: 'app-deviation-timeline',
  template: '<canvas class="timeline-canvas" #canvas></canvas>',
  styles: `
    :host { display: block; }
    .timeline-canvas { width: 100%; height: 48px; display: block; }
  `,
})
export class DeviationTimelineComponent implements OnInit, OnDestroy {
  /** Max |z| per snapshot, oldest → newest. */
  readonly values = input<number[]>([]);
  private readonly canvas =
    viewChild<ElementRef<HTMLCanvasElement>>('canvas');

  private ctx: CanvasRenderingContext2D | null = null;
  private raf = 0;
  private dirty = true;
  private latest: number[] = [];

  constructor() {
    effect(() => {
      this.latest = this.values();
      this.dirty = true;
    });
  }

  ngOnInit(): void {
    const el = this.canvas()?.nativeElement;
    if (el) {
      const dpr = window.devicePixelRatio || 1;
      el.width = Math.max(1, Math.round(el.clientWidth * dpr));
      el.height = Math.max(1, Math.round(el.clientHeight * dpr));
      this.ctx = el.getContext('2d');
      this.ctx?.setTransform(dpr, 0, 0, dpr, 0, 0);
    }
    const tick = () => {
      if (this.dirty) {
        this.draw();
        this.dirty = false;
      }
      this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  }

  ngOnDestroy(): void {
    cancelAnimationFrame(this.raf);
  }

  private draw(): void {
    const ctx = this.ctx;
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const w = ctx.canvas.width / dpr;
    const h = ctx.canvas.height / dpr;

    ctx.fillStyle = '#10151a';
    ctx.fillRect(0, 0, w, h);

    const maxShown = 8;
    const yFor = (z: number) => h - 3 - (Math.min(z, maxShown) / maxShown) * (h - 6);

    // Reference lines: |z| = 2 (ordinary noise ceiling) and 4 (alert).
    ctx.strokeStyle = '#2c3844';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, yFor(2) + 0.5);
    ctx.lineTo(w, yFor(2) + 0.5);
    ctx.stroke();
    ctx.strokeStyle = '#6b2c2c';
    ctx.beginPath();
    ctx.moveTo(0, yFor(4) + 0.5);
    ctx.lineTo(w, yFor(4) + 0.5);
    ctx.stroke();

    const values = this.latest;
    if (values.length < 2) return;

    const step = w / Math.max(values.length - 1, 1);
    ctx.strokeStyle = '#4caf9d';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    for (let i = 0; i < values.length; i++) {
      const x = w - (values.length - 1 - i) * step;
      const y = yFor(values[i]);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
  }
}
