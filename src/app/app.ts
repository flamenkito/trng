import { Component, OnInit, inject } from '@angular/core';
import { UpperCasePipe } from '@angular/common';
import { TrngService } from './core/trng.service';
import { TauriService } from './core/tauri.service';

@Component({
  selector: 'app-root',
  imports: [UpperCasePipe],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App implements OnInit {
  private readonly tauri = inject(TauriService);
  private readonly trng = inject(TrngService);

  protected readonly status = this.tauri.backendStatus;
  protected readonly testEvent = this.tauri.lastEvent;
  protected readonly stream = this.trng.streamStatus;

  ngOnInit(): void {
    void this.tauri.subscribeToTestEvent();
    void this.tauri.loadBackendStatus();
    void this.trng.subscribe();
  }

  protected refresh(): void {
    void this.tauri.loadBackendStatus();
  }

  protected formatRate(bytesPerSecond: number): string {
    if (bytesPerSecond >= 1024) {
      return `${(bytesPerSecond / 1024).toFixed(1)} KB/s`;
    }
    return `${bytesPerSecond.toFixed(0)} B/s`;
  }
}
