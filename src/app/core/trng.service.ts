import { Injectable, signal } from '@angular/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { StreamStatus } from '../models/stream-status';

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
