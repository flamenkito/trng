import { Component } from '@angular/core';
import { LivePageComponent } from './live/live-page.component';

@Component({
  selector: 'app-root',
  imports: [LivePageComponent],
  template: '<app-live-page />',
})
export class App {}
