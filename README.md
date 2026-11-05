# Infinite Noise TRNG Visualizer

Native macOS desktop app that reads an Infinite Noise TRNG via the official
`infnoise` executable, computes exact rolling-window statistics, and renders
deviations from a uniform distribution in real time.

Built with **Tauri 2** (Rust backend) + **Angular** (standalone components,
signals, zoneless change detection, Canvas 2D for live visualization).

> Status: **Milestone 1 — project skeleton.** No TRNG integration yet.

## Development commands

```bash
npm install                        # frontend dependencies

npm start                          # Angular dev server on http://localhost:4200
                                   # (UI only; Tauri IPC unavailable outside the app)

npm run tauri dev                  # full Tauri app: Rust backend + Angular dev server
npm run tauri build                # production build (Rust + Angular)

npm test                           # Angular unit tests (vitest, single run)
npm run build                      # Angular production build -> dist/trng/browser

cd src-tauri && cargo test         # Rust unit tests
cd src-tauri && cargo check        # fast Rust type check
```

## Milestone 1 acceptance

The skeleton proves the Rust ↔ Angular communication path:

- Angular invokes the Rust command `get_backend_status` and displays the
  returned payload (app name, versions, status).
- Rust emits a one-shot `backend-test-event` ~500 ms after startup; Angular
  receives it and displays the message.

## Project structure

```text
src/                        Angular frontend
  app/
    app.ts / app.html       root component (main screen)
    core/
      tauri.service.ts      thin wrapper over the Tauri IPC bridge
                            (invoke + event listen, signal-based)
  main.ts                   bootstrap (zoneless)

src-tauri/                  Rust backend
  src/
    main.rs                 binary entry point
    lib.rs                  Tauri builder, plugin + command registration
    ipc/
      mod.rs
      commands.rs           get_backend_status command
      events.rs             backend-test-event emitter
  tauri.conf.json           Tauri configuration
```

Future modules (device/, stream/, rolling/, capture/) land in Milestones 2–4.

## Architecture invariants (apply from Milestone 2 onward)

1. Every byte received from the TRNG is processed exactly once — the UI
   refresh rate never determines how much input data is processed.
2. One authoritative stream source; downstream consumers (capture,
   statistics) are fed from the internal pipeline.
3. All internal queues are bounded; any application-level drop is an explicit,
   visible error — never silently continue.
4. `stream.bin` captures are byte-for-byte TRNG output; no headers, no text
   encoding, no transformation.
5. Frontend snapshots (10–30 Hz) may be skipped; input bytes may not.
6. Distinguish application-level integrity from upstream
   device/driver/FTDI integrity; never claim an absolute "DROPPED: 0".

## Source modes

Initial mode is `WHITENED` (normal `infnoise` output). Raw-source mode is a
later milestone and must not be assumed to be uniformly distributed.
