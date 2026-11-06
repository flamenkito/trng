import {
  Component,
  ElementRef,
  OnDestroy,
  OnInit,
  effect,
  input,
  viewChild,
} from '@angular/core';
import { HeatmapRenderer } from '../../rendering/heatmap-renderer';

/**
 * Scrolling deviation heatmap (byte value over time). Appends a column per
 * incoming frame; skips are harmless — history simply advances faster.
 */
@Component({
  selector: 'app-history-heatmap',
  template: '<canvas class="heatmap-canvas" #canvas></canvas>',
  styles: `
    :host { display: block; }
    .heatmap-canvas { width: 100%; height: 160px; display: block; }
  `,
})
export class HistoryHeatmapComponent implements OnInit, OnDestroy {
  readonly counts = input<number[] | null>(null);
  readonly samples = input(0);
  private readonly canvas =
    viewChild<ElementRef<HTMLCanvasElement>>('canvas');

  private readonly renderer = new HeatmapRenderer();

  constructor() {
    effect(() => {
      const counts = this.counts();
      if (counts && counts.length === 256) {
        this.renderer.push(counts, this.samples());
      }
    });
  }

  ngOnInit(): void {
    const el = this.canvas()?.nativeElement;
    if (el) {
      this.renderer.attach(el);
    }
    this.renderer.start();
  }

  ngOnDestroy(): void {
    this.renderer.stop();
  }

  /** Pins a labeled experiment marker at the current time position. */
  markEvent(label: string): void {
    this.renderer.addMarker(label);
  }
}
