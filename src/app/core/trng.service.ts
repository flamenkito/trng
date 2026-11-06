import { Injectable, signal } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { StreamStatus } from '../models/stream-status';

export interface CaptureActionResult {
  dir: string;
  bytesWritten: number;
}

/**
 * Consumes `stream-status` snapshots from the Rust stream pipeline and
 * exposes them as signals.
 *
 * The service only ever holds the *latest* snapshot: frontend frames may be
 * skipped, TRNG bytes may not.
 */
@Injectable({ providedIn: 'root' })
export class TrngService {
  readonly streamStatus = signal<StreamStatus | null>(null);

  private unlisten: UnlistenFn | null = null;

  /** Starts a lossless binary capture; resolves with the capture directory. */
  async startCapture(): Promise<CaptureActionResult> {
    return await invoke<CaptureActionResult>('start_capture');
  }

  /** Stops the capture; resolves with the directory and exact byte count. */
  async stopCapture(): Promise<CaptureActionResult> {
    return await invoke<CaptureActionResult>('stop_capture');
  }

  async subscribe(): Promise<void> {
    if (this.unlisten) {
      return;
    }
    try {
      this.unlisten = await listen<StreamStatus>('stream-status', (event) =>
        this.streamStatus.set(event.payload),
      );
    } catch {
      /* not running inside Tauri */
    }
  }
}
