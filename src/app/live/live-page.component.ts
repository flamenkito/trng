import { Component, OnInit, computed, inject, signal } from '@angular/core';
import { UpperCasePipe } from '@angular/common';
import { TrngService } from '../core/trng.service';
import { TauriService } from '../core/tauri.service';
import { DeviationCanvasComponent } from '../components/deviation-canvas/deviation-canvas.component';
import { HistoryHeatmapComponent } from '../components/history-heatmap/history-heatmap.component';
import { WindowSelectorComponent } from '../components/window-selector/window-selector.component';
import { toFrame } from '../models/distribution-frame';

/**
 * Primary live screen: device status, deviation graph, window selection,
 * history heatmap, and capture controls. Business/data acquisition lives in
 * services; this component only orchestrates presentation.
 */
@Component({
  selector: 'app-live-page',
  imports: [
    UpperCasePipe,
    DeviationCanvasComponent,
    HistoryHeatmapComponent,
    WindowSelectorComponent,
  ],
  templateUrl: './live-page.component.html',
  styleUrl: './live-page.component.css',
})
export class LivePageComponent implements OnInit {
  private readonly tauri = inject(TauriService);
  private readonly trng = inject(TrngService);

  protected readonly status = this.tauri.backendStatus;
  protected readonly stream = this.trng.streamStatus;

  /** UI-only state: which rolling window feeds the graphs. */
  protected readonly selectedWindowMs = signal(1000);

  /** Frame for the selected window; null until enough samples exist. */
  protected readonly selectedFrame = computed(() => {
    const s = this.stream();
    if (!s) return null;
    const stats = s.windows.find((w) => w.windowMs === this.selectedWindowMs());
    return toFrame(stats);
  });

  ngOnInit(): void {
    void this.tauri.loadBackendStatus();
    void this.trng.subscribe();
  }

  protected onSelectWindow(ms: number): void {
    this.selectedWindowMs.set(ms);
  }

  protected formatRate(bytesPerSecond: number): string {
    if (bytesPerSecond >= 1024) {
      return `${(bytesPerSecond / 1024).toFixed(1)} KB/s`;
    }
    return `${bytesPerSecond.toFixed(0)} B/s`;
  }

  protected startCapture(): void {
    void this.trng.startCapture().catch((e: unknown) => console.error(e));
  }

  protected stopCapture(): void {
    void this.trng.stopCapture().catch((e: unknown) => console.error(e));
  }
}
