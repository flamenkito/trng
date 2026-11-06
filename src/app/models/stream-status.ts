import type { DeviceStatus } from './device-status';

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
}
