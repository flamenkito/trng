import {
  Component,
  ElementRef,
  OnDestroy,
  OnInit,
  effect,
  input,
  viewChild,
} from '@angular/core';
import type { DistributionFrame } from '../../models/distribution-frame';
import { DeviationRenderer } from '../../rendering/deviation-renderer';

/**
 * Hosts the deviation-from-uniformity canvas. Angular owns the element and
 * lifecycle; the renderer owns drawing via requestAnimationFrame. Drawing
 * never triggers change detection.
 */
@Component({
  selector: 'app-deviation-canvas',
  template: '<canvas class="deviation-canvas" #canvas></canvas>',
  styles: `
    :host { display: block; }
    .deviation-canvas { width: 100%; height: 260px; display: block; }
  `,
})
export class DeviationCanvasComponent implements OnInit, OnDestroy {
  /** One frame per rolling window, shortest first (rendered front to back). */
  readonly frames = input<DistributionFrame[]>([]);
  readonly selectedMs = input(1000);
  private readonly canvas =
    viewChild<ElementRef<HTMLCanvasElement>>('canvas');

  private readonly renderer = new DeviationRenderer();

  constructor() {
    // React to new frames outside the render loop: mark dirty only.
    effect(() => {
      this.renderer.setFrames(this.frames(), this.selectedMs());
      this.renderer.invalidate();
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
}
