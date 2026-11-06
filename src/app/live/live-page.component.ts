import { Component, OnInit, computed, effect, inject, signal, viewChild } from '@angular/core';
import { UpperCasePipe } from '@angular/common';
import { TrngService, type SourceRequest } from '../core/trng.service';
import { TauriService } from '../core/tauri.service';
import { DeviationCanvasComponent } from '../components/deviation-canvas/deviation-canvas.component';
import { DeviationTimelineComponent } from '../components/deviation-timeline/deviation-timeline.component';
import { HistoryHeatmapComponent } from '../components/history-heatmap/history-heatmap.component';
import { WindowSelectorComponent } from '../components/window-selector/window-selector.component';
import { binDeviation, toFrame, type DistributionFrame } from '../models/distribution-frame';

/** A bin that has stayed significantly elevated long enough to flag. */
export interface SustainedDeviation {
  bin: number;
  windowMs: number;
  ticks: number;
  maxZ: number;
}

/** ~10 Hz snapshots; 600 ticks = 60 s of timeline. */
const TIMELINE_CAPACITY = 600;
/** |z| above this is "significant" for the sustained-deviation alert. */
const ALERT_Z = 4.0;
/** Significant ticks required before alerting (30 ≈ 3 s at 10 Hz). */
const ALERT_TICKS = 30;

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
    DeviationTimelineComponent,
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

  /** Path of a capture file to replay (UI-only state). */
  protected readonly replayPath = signal('');

  /** Which source spec is active, for button highlighting. */
  protected readonly activeSource = computed(() => {
    const source = this.stream()?.source ?? '';
    if (source.startsWith('replay:')) return 'replay';
    if (source.startsWith('testpattern')) return 'testPattern';
    if (source.startsWith('infnoise (raw')) return 'infnoiseRaw';
    return 'infnoise';
  });

  /** Frame for the selected window; null until enough samples exist. */
  protected readonly selectedFrame = computed(() => {
    const s = this.stream();
    if (!s) return null;
    const stats = s.windows.find((w) => w.windowMs === this.selectedWindowMs());
    return toFrame(stats);
  });

  /** All window frames, shortest first — feeds the deviation waterfall. */
  protected readonly allFrames = computed<DistributionFrame[]>(() => {
    const s = this.stream();
    if (!s) return [];
    return s.windows
      .map((w) => toFrame(w))
      .filter((f): f is DistributionFrame => f !== null);
  });

  /** Max |z| per snapshot, oldest → newest; feeds the timeline strip. */
  protected readonly deviationTimeline = signal<number[]>([]);

  /** Set while any bin is persistently elevated — possible external influence. */
  protected readonly sustainedAlert = signal<SustainedDeviation | null>(null);

  /** UI state: experiment marker label and recorded marker log. */
  protected readonly markerLabel = signal('');
  protected readonly markerLog = signal<{ time: string; label: string }[]>([]);
  private heatmapRef = viewChild(HistoryHeatmapComponent);

  // Sustained-deviation detector state (plain fields; tick-driven).
  private streak = 0;
  private streakBin = -1;
  private alerting = false;

  constructor() {
    // Per-snapshot detection: max |z| over the longest populated window,
    // plus a streak counter implementing the persistence rule — random
    // noise breaks a streak within a tick or two; real influence holds.
    effect(() => {
      const s = this.stream();
      if (!s) return;
      const longest = [...s.windows]
        .sort((a, b) => b.windowMs - a.windowMs)
        .find((w) => w.samples >= 256);
      if (!longest) return;

      let maxZ = 0;
      let maxBin = -1;
      for (let i = 0; i < 256; i++) {
        const z = Math.abs(binDeviation(longest.counts[i], longest.samples));
        if (z > maxZ) {
          maxZ = z;
          maxBin = i;
        }
      }

      const timeline = this.deviationTimeline();
      const next =
        timeline.length >= TIMELINE_CAPACITY
          ? [...timeline.slice(1), maxZ]
          : [...timeline, maxZ];
      this.deviationTimeline.set(next);

      if (maxZ >= ALERT_Z) {
        if (maxBin === this.streakBin) {
          this.streak++;
        } else {
          this.streakBin = maxBin;
          this.streak = 1;
        }
      } else if (this.streak > 0) {
        this.streak--;
        if (this.streak === 0) this.streakBin = -1;
      }

      if (!this.alerting && this.streak >= ALERT_TICKS) {
        this.alerting = true;
        this.sustainedAlert.set({
          bin: this.streakBin,
          windowMs: longest.windowMs,
          ticks: this.streak,
          maxZ,
        });
      } else if (this.alerting) {
        if (this.streak === 0) {
          this.alerting = false;
          this.sustainedAlert.set(null);
        } else {
          this.sustainedAlert.set({
            bin: this.streakBin,
            windowMs: longest.windowMs,
            ticks: this.streak,
            maxZ,
          });
        }
      }
    });
  }

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

  protected switchSource(request: SourceRequest): void {
    void this.trng.setSource(request).catch((e: unknown) => console.error(e));
  }

  protected onReplayPath(event: Event): void {
    this.replayPath.set((event.target as HTMLInputElement).value);
  }

  protected startReplay(): void {
    const path = this.replayPath().trim();
    if (path) {
      this.switchSource({ type: 'replay', path });
    }
  }

  protected onMarkerLabel(event: Event): void {
    this.markerLabel.set((event.target as HTMLInputElement).value);
  }

  /** Records a marker on the heatmap and in the wall-clock log. */
  protected addMarker(): void {
    const label =
      this.markerLabel().trim() ||
      `MARK ${this.markerLog().length + 1}`;
    const time = new Date().toLocaleTimeString('en-GB', { hour12: false });
    this.heatmapRef()?.markEvent(label);
    this.markerLog.update((log) => [...log, { time, label }]);
    this.markerLabel.set('');
  }
}
