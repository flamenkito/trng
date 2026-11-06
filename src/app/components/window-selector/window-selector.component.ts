import { Component, input, output } from '@angular/core';

/** Rolling-window selector: [1s] [5s] [30s] [1m] [5m]. */
@Component({
  selector: 'app-window-selector',
  template: `
    <div class="selector">
      @for (w of windows; track w.ms) {
        <button
          type="button"
          [class.selected]="w.ms === selectedMs()"
          (click)="select.emit(w.ms)"
        >
          {{ w.label }}
        </button>
      }
    </div>
  `,
  styles: `
    .selector { display: flex; gap: 6px; }
    button {
      background: #1b232b;
      border: 1px solid #2c3844;
      color: #8fa1b0;
      border-radius: 5px;
      padding: 5px 12px;
      cursor: pointer;
      font-size: 12px;
    }
    button:hover { background: #223041; }
    button.selected {
      background: #223041;
      border-color: #4c9baf;
      color: #d7dde3;
    }
  `,
})
export class WindowSelectorComponent {
  readonly windows = [
    { ms: 1000, label: '1s' },
    { ms: 5000, label: '5s' },
    { ms: 30000, label: '30s' },
    { ms: 60000, label: '1m' },
    { ms: 300000, label: '5m' },
  ];
  readonly selectedMs = input(1000);
  readonly select = output<number>();
}
