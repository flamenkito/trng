import { Injectable, signal } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface BackendStatus {
  appName: string;
  appVersion: string;
  tauriVersion: string;
  status: string;
  timestampMs: number;
}

export interface BackendTestEvent {
  message: string;
  timestampMs: number;
}

/**
 * Thin wrapper around the Tauri IPC bridge.
 *
 * Kept separate from the future TrngService so acquisition logic never
 * leaks into presentation components.
 */
@Injectable({ providedIn: 'root' })
export class TauriService {
  readonly backendStatus = signal<BackendStatus | null>(null);
  readonly lastEvent = signal<BackendTestEvent | null>(null);

  private eventUnlisten: UnlistenFn | null = null;

  /**
   * Invokes the Rust status command and stores the result in a signal.
   *
   * Silently no-ops when the app runs outside the Tauri shell
   * (unit tests, plain `ng serve`), leaving the signal null.
   */
  async loadBackendStatus(): Promise<void> {
    try {
      const status = await invoke<BackendStatus>('get_backend_status');
      this.backendStatus.set(status);
    } catch {
      /* not running inside Tauri */
    }
  }

  /**
   * Subscribes to the one-shot Rust test event.
   *
   * Silently no-ops when the app runs outside the Tauri shell.
   */
  async subscribeToTestEvent(): Promise<void> {
    if (this.eventUnlisten) {
      return;
    }
    try {
      this.eventUnlisten = await listen<BackendTestEvent>(
        'backend-test-event',
        (event) => this.lastEvent.set(event.payload),
      );
      // Subscribe first, then ask Rust to emit so we never miss the event.
      await invoke('request_test_event');
    } catch {
      /* not running inside Tauri */
    }
  }
}
