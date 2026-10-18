# Sysarium

A live Linux CPU monitor with a friendly computer, clear numbers, and short histories.

```sh
cargo install --path apps/sysarium --locked
sysarium
# Or: cargo run --release -p sysarium
```

Requires Linux, Rust 1.88+, and a UTF-8 terminal of at least **80 × 24**. SSH with a PTY works too. `sysarium live` opens the same monitor.

**Relaxed** · CPU **8%** · wait **0.1%**

![Relaxed computer — synthetic data](docs/assets/relaxed.png)

| Busy · little waiting | Busy · CPU contention |
|---|---|
| ![CPU 93%, wait 0.2%](docs/assets/busy-low-wait.gif) | ![CPU 92%, wait 28%](docs/assets/busy-high-wait.gif) |
| CPU **93%** · wait **0.2%** | CPU **92%** · wait **28%** |

**Strained** · CPU **99%** · wait **28%**

![Strained computer — synthetic data](docs/assets/strained.gif)

Synthetic fixtures, not a load test. **Green = relaxed · amber = busy · red = strained.** Character state follows smoothed CPU usage; it does not measure latency. Papers reflect CPU wait. [Static previews](docs/assets/busy-low-wait.png) · [with contention](docs/assets/busy-high-wait.png).

| Control | Action |
|---|---|
| `Tab` / `1` · `2` · `3` | Switch view: Animation / Charts / Both |
| `i` / `?` | Details / help |
| `Esc` | Back |
| `↑↓` / `PgUp` / `PgDn` / `Home` | Scroll |
| `q` / `Ctrl-C` | Quit |

**Animation** shows the character and papers. **Both** adds charts alongside it. Smaller terminals show a resize prompt and live values. Resizing back restores the full view; monitoring continues throughout.

| Metric | Meaning |
|---|---|
| CPU usage · Working | CPU time spent doing work, across all CPUs (approx.). |
| CPU wait · Waiting for CPU | Time with at least one task waiting for CPU, measured by Linux PSI some. |
| Wait time | Milliseconds with a task waiting, within the latest sample. |
| 10s trend | Recent CPU wait, smoothed over roughly 10 seconds. |
| Runnable | Running and ready tasks; not a waiting count. |

Papers show waiting time, not a task count.

**PSI** is Linux's CPU-wait measure. Details make it concrete: **28% wait = 280 ms in a 1 s sample**. Usage is an approximation, excluding I/O wait and VM steal; [calculation rules](docs/ARCHITECTURE.md) and `--json` retain the formulas and counters.

Updates run at ~1 Hz, animation at up to 10 FPS. Charts keep up to 120 readings, past on the left, latest on the right, on a 0–100% scale. `N/A` and `?` mean unknown; `▁` marks zero or very low values. The first reading needs about one second for a difference. Missing data never becomes zero.

```sh
sysarium --json > observations.ndjson
# Ctrl-C to stop
```

One JSON record is flushed per reading, including the first. Schema v1 retains units, sample IDs, raw counters, formulas, and complete failure reasons. Diagnostics go to stderr. Closing a pipe exits cleanly. Only the visible `/proc` scope is observed; container and VM scope may differ from the physical host.

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 apps/sysarium/tests/verify_terminal.py
cargo build --release --locked -p sysarium
```

The workspace has three crates: OS-independent `inside-core`, procfs I/O in `inside-linux`, and the `sysarium` app. Tests use fixtures and PTYs; no automated high-load workload is started. Manual load comparison and user evaluation remain open. See [validation](docs/P3-VALIDATION.md), [decisions](docs/DECISIONS.md), and [release checks](docs/RELEASE.md).

The app was renamed from `inside` before release. Install and run `sysarium`; internal library names remain unchanged. GUI, Agent, Simulation, and long-term storage are outside v0.1.
