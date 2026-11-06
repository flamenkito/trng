import type { DeviceStatus } from './device-status';
import type { CaptureStatus } from './capture-status';

/** Exact per-value counts for one rolling window. */
export interface WindowStats {
  windowMs: number;
  samples: number;
  counts: number[];
}

/**
 * Snapshot of derived stream state, emitted by Rust at ~10 Hz.
 *
 * `applicationDroppedBytes` reflects application-level integrity only — it
 * says nothing about losses inside the USB device, FTDI transport, OS
 * driver, or infnoise, none of which provide sequence information.
 */
export interface StreamStatus {
  deviceStatus: DeviceStatus;
  source: string;
  totalBytesReceived: number;
  bytesPerSecond: number;
  applicationDroppedBytes: number;
  chunksProcessed: number;
  reconnectCount: number;
  lastError: string | null;
  captureStatus: CaptureStatus;
  captureBytesWritten: number;
  captureDir: string | null;
  captureError: string | null;
  /** Distribution counts for every configured rolling window. */
  windows: WindowStats[];
}
