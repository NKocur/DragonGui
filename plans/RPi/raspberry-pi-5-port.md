# Raspberry Pi 5 Port

## Goal

Ship a `dragongui` build that runs on a Raspberry Pi 5 (aarch64, Pi OS
bookworm or newer, V3D GPU) without forking the codebase. Desktop builds
stay byte-for-byte unchanged; the Pi build is gated behind a Cargo
feature and a separate CI wheel.

## Non-goals

- Feature parity with desktop. The Pi's V3D GPU cannot match a discrete
  GPU on point-cloud throughput, MSAA, or compute paths.
- 32-bit Pi (armv7) support. Pi 5 is aarch64-only and earlier Pis are
  out of scope for this plan.
- Headless / framebuffer-only operation. The plan assumes a Wayland or
  X11 session.

## Constraints we have to design around

- **GPU:** V3D VII, Vulkan ~1.2 with gaps. wgpu often picks GL as a
  fallback. Compute shaders are limited; storage-buffer size in the
  vertex stage is small.
- **Drivers:** mesa version on Pi OS lags upstream. Behaviour changes
  between point releases — we must pin a known-good combo in install
  docs.
- **Memory:** 4 GB / 8 GB / 16 GB SKUs. The Rust + lightningcss build
  needs ≥8 GB to compile comfortably; users on 4 GB should install a
  prebuilt wheel, not build from source.
- **No published aarch64-linux wheel today.** This plan introduces one.

## Where the code needs to change

### 1. Cargo feature flag

[native/Cargo.toml](../../native/Cargo.toml)

- Add a `pi` feature under `[features]`. Off by default. Enabling it
  flips the runtime backend and limit selection (see §2).
- No new dependencies are required for the Pi target. `rfd` 0.15
  already uses xdg-portal on Linux. The Windows-only deps are correctly
  gated by `cfg(windows)` and will not be pulled in.

[pyproject.toml](../../pyproject.toml)

- Update `[tool.maturin]` so the Pi CI job can pass
  `--features dragongui-native/pi` through `cargo-extra-args`, or
  expose it as a maturin profile.

### 2. wgpu instance, adapter, and device

[native/src/runtime.rs:4991](../../native/src/runtime.rs#L4991)

Currently:

```rust
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
// power_preference: HighPerformance
// required_limits: wgpu::Limits::default()
```

Changes when `pi` feature is on (or always when
`cfg(all(target_arch = "aarch64", target_os = "linux"))`):

- `InstanceDescriptor { backends: Backends::VULKAN | Backends::GL, .. }`
  so wgpu can fall back to GL when V3D's Vulkan path misbehaves.
- `power_preference: LowPower`.
- Replace `Limits::default()` with
  `Limits::downlevel_defaults().using_resolution(adapter.limits())`.
  Without this, device creation fails on V3D.
- Keep `force_fallback_adapter: false` — the V3D adapter is the real
  one we want.

### 3. Shader audit

[native/src/primitives/rect.wgsl](../../native/src/primitives/rect.wgsl)
and every other `.wgsl` under [native/src/](../../native/src/).

Watch for and fix:

- `textureSampleLevel` with non-zero LOD on textures without mip chains.
- Storage buffers bound to the vertex stage (scatter does this) —
  chunk uploads to stay under V3D's small per-binding limit.
- `f16` or packed types — replace with `f32`.

Action: build with `pi` feature, run each example in
[examples/css_feature_probes/](../../examples/css_feature_probes/) on
real hardware, capture validation errors, fix iteratively.

### 4. Heavy widgets — lower the ceiling

[native/src/scatter/mod.rs](../../native/src/scatter/mod.rs)

- When `pi` is on: cap max points at 100 000 (vs. 500 000+),
  disable MSAA, force a coarser LOD threshold, smaller upload chunks.

[native/src/table.rs](../../native/src/table.rs)

- Smaller column buffer sizes; lower row prefetch window.

These should be `cfg!(feature = "pi")` guards on the constants, not
runtime branches in hot paths.

### 5. WebView widget

[native/src/html_report_webview.rs](../../native/src/html_report_webview.rs)
is `cfg(windows)`-only. On Pi the widget does not exist.

[python/dragongui/widgets.py](../../python/dragongui/widgets.py)

- The Python-side `HtmlReport` (or equivalent) must raise a clear
  `NotImplementedError("HtmlReport is not supported on this platform")`
  rather than crashing the native side.

### 6. File dialogs

No code change needed. Document that
`xdg-desktop-portal-gtk` (or `-gnome` / `-kde`) must be installed on
the Pi for `rfd` to work.

## Build & distribution

### Local build (developer Pi)

Pi OS bookworm, 8 GB+ RAM:

```
sudo apt install build-essential cmake libvulkan1 mesa-vulkan-drivers \
                 libxkbcommon-dev libwayland-dev xdg-desktop-portal-gtk \
                 python3.11-dev python3-pip
curl https://sh.rustup.rs -sSf | sh
pip install maturin
maturin build --release --features dragongui-native/pi
pip install target/wheels/dragongui-*.whl
```

### CI wheel

GitHub Actions:

- New job `build-pi-wheel` on `ubuntu-22.04-arm` runner (native aarch64,
  no QEMU).
- Uses `PyO3/maturin-action` with
  `args: --release --features dragongui-native/pi` and
  `manylinux: manylinux_2_34`.
- Produces `dragongui-<ver>-cp311-abi3-manylinux_2_34_aarch64.whl`.
- Publishes to PyPI alongside the desktop wheels — same package name,
  different platform tag, so `pip install dragongui` Just Works on a Pi.

Fallback if `ubuntu-22.04-arm` is unavailable: `cross` with the
`aarch64-unknown-linux-gnu` target. Slower, but no QEMU emulation of
the wheel itself.

## Testing

- **Smoke test on real hardware.** Required. There is no substitute for
  V3D-on-mesa behaviour. Maintain a Pi 5 in CI or a manual pre-release
  checklist.
- **Examples to run:**
  [examples/all_features_v3_demo.py](../../examples/all_features_v3_demo.py),
  every probe under
  [examples/css_feature_probes/](../../examples/css_feature_probes/).
- **Pinned baseline:** record the kernel + mesa + Pi OS image used
  during validation in this plan as the supported baseline. Update it
  per release.

## Phased rollout

1. **Phase 1 — boots.** Feature flag, downlevel limits, GL fallback.
   Goal: a window opens with a `Button` and `Label` on the Pi.
2. **Phase 2 — widgets.** Shader audit; all 45+ widgets render
   correctly. No scatter / table tuning yet.
3. **Phase 3 — heavy widgets.** Scatter and table ceilings tuned;
   `HtmlReport` raises cleanly.
4. **Phase 4 — distribution.** CI wheel published to PyPI; install
   docs + supported-baseline note added to README.
5. **Phase 5 — maintenance.** Add Pi smoke-test step to release
   checklist; track wgpu / winit / mesa upgrades against real
   hardware.

Phases 1–3 are the engineering work. Phase 1 alone tells us whether
this is a 1-week job or a 1-month job.

## Risks

- **V3D Vulkan regressions across mesa versions.** Mitigation: pin a
  baseline, document it, retest on upgrades.
- **wgpu major releases drop downlevel paths.** Mitigation: the Pi
  build pins wgpu carefully; bumping wgpu requires a Pi smoke test.
- **CI runner availability for aarch64 Linux.** Mitigation: `cross`
  fallback documented above.
- **User confusion about reduced ceilings.** Mitigation: Pi-specific
  section in [docs/widgets.md](../../docs/widgets.md) listing the
  caps (max points, no MSAA, no `HtmlReport`).

## Open questions

- Do we ship one wheel that auto-detects the Pi at runtime, or keep a
  separate `pi`-feature wheel? Current plan: separate wheel, same
  package name, selected by platform tag. Simpler, no runtime cost on
  desktop. Revisit if PyPI tag resolution causes friction.
- Pi 4 support? Out of scope here, but most of this plan also applies
  if someone wants to extend it later — the limits would need to drop
  further.
