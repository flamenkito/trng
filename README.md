# Infinite Noise TRNG Visualizer

Native macOS desktop app that reads an Infinite Noise TRNG via the official
`infnoise` executable, computes exact rolling-window statistics, and renders
deviations from a uniform distribution in real time.

Built with **Tauri 2** (Rust backend) + **Angular** (standalone components,
signals, zoneless change detection, Canvas 2D for live visualization).

> Status: **Milestones 1–9 complete.** Live TRNG streaming, lossless capture,
> rolling-window distributions, deviation + heatmap visualization, replay,
> and raw-source mode. Experiment markers and polish are future work.

## Device prerequisites (macOS)

The app drives the TRNG through the official `infnoise` executable
(WHITENED mode, binary stdout).

```bash
brew install libftdi
git clone https://github.com/waywardgeek/infnoise.git
cd infnoise/software && make -f Makefile.darwin
install_name_tool -add_rpath /opt/homebrew/opt/libftdi/lib infnoise
install_name_tool -add_rpath /opt/homebrew/lib infnoise
cp infnoise /opt/homebrew/bin/
```

> **Note on the Apple FTDI driver:** since macOS Catalina the Apple FTDI
> driver is a DriverKit system extension that cannot be unloaded — but it
> does *not* claim the device exclusively, so `infnoise`/`libftdi` work
> alongside it. The historical `sudo kextunload` recipes are obsolete.
>
> **Note on sandboxes:** sandboxed processes get no USB device access and
> `infnoise` fails with "Can't open Infinite Noise Multiplier". Run the app
> unsandboxed (a normal Terminal is fine); no sudo needed.

For development without the physical TRNG:

```bash
TRNG_SOURCE=testpattern npm run tauri dev   # deterministic 00..FF loop source
```

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
      trng.service.ts       consumes `stream-status` snapshots into signals
    models/
      device-status.ts
      stream-status.ts
  main.ts                   bootstrap (zoneless)

src-tauri/                  Rust backend
  src/
    main.rs                 binary entry point
    lib.rs                  Tauri builder, pipeline startup and shutdown
    ipc/
      mod.rs
      commands.rs           get_backend_status command
      events.rs             backend-test-event + stream-status emitters
    device/
      mod.rs                ByteSource trait (one authoritative stream reader)
      infnoise.rs           infnoise subprocess source (whitened, binary)
      test_pattern.rs       deterministic test source (00..FF loop)
    stream/
      mod.rs
      counters.rs           exact byte counters + offset-integrity verifier
      processor.rs          reader/processor/ticker threads, bounded channel,
                            reconnect with visible error state
    state.rs                shared stream state (status, errors, counters)
  tauri.conf.json           Tauri configuration
```

Future modules (rolling/, capture/) land in Milestones 3–4.

## How it works

### Data flow

```text
Infinite Noise TRNG (USB)
        │  infnoise (whitened: raw noise conditioned through a Keccak sponge)
        ▼
   infnoise stdout  ← one continuous byte stream; chunk boundaries are
                      pipe-buffering artifacts and carry no meaning
        ▼
┌─ READER THREAD ─────────────────────────────────────────────┐
│ blocking reads into a 64 KB buffer; pushes into a bounded    │
│ channel (4096 chunks). Under backpressure the reader BLOCKS  │
│ — it never drops a byte.                                     │
└────────────────────────┬────────────────────────────────────┘
                         ▼
┌─ PROCESSOR THREAD (single authoritative consumer) ──────────┐
│ every chunk processed exactly once, in arrival order:        │
│  1. OffsetVerifier — any gap/overlap in stream offsets       │
│     becomes an explicit, visible error                       │
│  2. StreamCounters — exact total byte count                  │
│  3. CaptureWriter — appends bytes to stream.bin (no headers, │
│     no transformation)                                       │
│  4. RollingHistogram — every byte increments exactly one of  │
│     256 bins in a 100 ms timestamped bucket                  │
└────────────────────────┬────────────────────────────────────┘
                         ▼
┌─ TICKER (10 Hz) ────────────────────────────────────────────┐
│ publishes `stream-status` snapshots: totals, exact byte rate │
│ (1 s sliding window), queue depth, capture state, and        │
│ counts[256] for every configured rolling window              │
└────────────────────────┬────────────────────────────────────┘
                         ▼
              Angular signals → Canvas 2D renderers (rAF)
```

The critical architectural property: **the UI refresh rate is decoupled from
data processing.** Bytes flow through the pipeline regardless of what the
frontend does; snapshots are derived state that may be skipped without
affecting acquisition (Rule 5).

### Rolling windows

The processor aggregates the stream into **100 ms timestamped buckets**, each
holding `counts[256]` plus an implicit byte total. A window of duration W
queried at time T is the exact sum of all buckets whose start falls in
`(T − W, T]`, plus the in-progress bucket. Documented semantics (bucket
approximation per spec §11): because bucket boundaries are fixed, the covered
span can exceed W by up to one bucket duration at the trailing edge — counts
are exact for the covered span; only the span edges are bucket-aligned.
Windows are never re-sampled: a 5-minute window is reconstructed from every
bucket covering it, so it contains every byte received during it.

### The deviation metric

The renderers do not plot raw counts. For a window with N samples, each byte
value is expected `E = N/256` times under uniformity; the plotted quantity
per bin is the z-score:

```text
z = (count − N/256) / √(N/256)
```

For a uniform source, z-scores follow a standard normal distribution:
**|z| ≤ ~2 is ordinary sampling noise; |z| > 4–5 is genuinely unusual**,
independent of sample size. This is what powers the visual design principle
(§17): ordinary noise stays below the `|z| ≤ 1.5` "quiet" threshold and
renders nothing; departures grow progressively taller and warmer in color.

### Detecting external influence

The TRNG's physical core is two noisy inverters whose metastable outputs are
amplified. Physical perturbations (EM fields, temperature, voltage supply,
vibration) can bias the raw noise *before* whitening. Detection depends on
mode:

1. **Deviation waterfall (whitened).** All rolling windows (1s…5m) are drawn
   as layers in depth — shortest at the front, longest at the back. A
   flickering bin lights up only in the front layers; real influence forms a
   *ridge* through all layers at the same byte value. The window selector
   highlights one layer instead of hiding the rest. Detection power scales
   with √N: with 5 minutes at ~35 KB/s (N ≈ 10M), even a 0.1% bias on one
   bin yields z ≈ 45, while ordinary noise is ±2.
2. **Sustained-deviation alert.** A detector tracks the maximum |z| per
   snapshot over the longest populated window; if one bin stays above
   z = 4 for ~3 s (~30 ticks), an alert banner names the byte value and
   duration. This automates the persistence rule: random noise breaks a
   streak within a tick or two, real influence holds.
3. **Deviation timeline + history heatmap.** The strip below the heatmap
   plots max |z| per snapshot (~60 s window): influence appears as a rise
   (onset), plateau (duration), and fall (recovery). The heatmap shows
   *which byte values* moved. Together they make temporal correlation
   trivial.
4. **Experiment markers.** Type a label (e.g. `MAGNET_ON`) and press ⚑ Mark
   at the moment of an intervention; a vertical line is pinned on the
   heatmap at that time (markers scroll with the data) and a wall-clock log
   is kept under the input. Markers never alter the captured byte stream.
5. **Raw mode (experimental).** The unwhitened source shows physical biases
   far more strongly — before conditioning they are much larger. If an
   influence exists at all, raw mode will amplify its visibility.

**Suggested protocol for an intervention experiment (heat / magnetic /
etc.):** start from a quiet baseline and watch the timeline sit under
z ≈ 2 → mark `BASELINE` → apply the influence → watch for an alert or a
timeline rise and note which bins ridge in the waterfall → remove the
influence → mark `OFF` → confirm the timeline falls back. A capture started
before the experiment preserves the raw stream for later replay through the
same pipeline.

#### Honest limitations

- **Deviation ≠ proof of cause.** The app shows statistical departure from
  uniformity, not its origin. Attribution requires experimental discipline:
  baseline capture → intervention → comparison. The capture + replay +
  heatmap combination exists exactly for this: capture a session with a
  known intervention window, replay it, and check the band timing.
- **Don't cherry-pick.** Watching 256 bins × 5 windows indefinitely will
  eventually produce a large |z| by pure chance (multiple comparisons). The
  built-in defense is that genuine signal *persists across growing windows*;
  coincidence does not.
- **Application integrity ≠ end-to-end integrity.** The app guarantees every
  byte delivered to it is processed and recorded. It cannot detect losses
  inside the USB device, FTDI transport, OS driver, or `infnoise`, because
  that path carries no sequence information (Rule 7). An upstream stall
  looks like a rate drop, not a distribution change.

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
