import { Component, OnInit, inject } from '@angular/core';
import { TauriService } from './core/tauri.service';

@Component({
  selector: 'app-root',
  imports: [],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App implements OnInit {
  private readonly tauri = inject(TauriService);

  protected readonly status = this.tauri.backendStatus;
  protected readonly testEvent = this.tauri.lastEvent;

  ngOnInit(): void {
    void this.tauri.subscribeToTestEvent();
    void this.tauri.loadBackendStatus();
  }

  protected refresh(): void {
    void this.tauri.loadBackendStatus();
  }
}
