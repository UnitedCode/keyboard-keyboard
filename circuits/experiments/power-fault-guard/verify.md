# Independently verify the power-fault guard

Use a clean checkout of the linked report revision, leaving your active firmware
branch alone. Check `df -h` and retain 500 GB free. Requires Rust, `shasum` and
native ngspice. On macOS: `brew install ngspice`; KiCad is optional for the drawing
check. Choose new result directories on every run. These commands never flash.

From the Keyboard Keyboard repository:

```sh
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR="$HOME/Documents/GitHub/synth-phone-e-v2-rust/target"
GUARD_HOST=$(rustc -vV | sed -n 's/^host: //p')
mkdir -p circuits/results
cargo test --locked --manifest-path tools/circuit-lab/Cargo.toml --target "$GUARD_HOST" --all-features
cargo build --release --locked --manifest-path tools/circuit-lab/Cargo.toml --target "$GUARD_HOST" --features device
GUARD_LAB="$CARGO_TARGET_DIR/$GUARD_HOST/release/keyboard-circuit-lab"
"$GUARD_LAB" guard-study circuits/baselines/keyboard-current.net \
  circuits/experiments/power-fault-guard/candidate.net circuits/results/verify-guard-1
"$GUARD_LAB" power-kernel circuits/results/verify-original-fault-1
```

Expected: `original_fault_failures=8`, `fixed_original_fault_failures=0`,
`guarded_fault_cases=3600`, `guarded_failures=0`, `unguarded_failure_controls=1788`.
The old `power-kernel` must still report **eight failed cases**. Its execution
passes by reproducing the failure, not by declaring the old allocation acceptable.
Read `startup-margin-report.txt`: one 3.168 V corner stays disarmed. This finding
must remain visible; a model-execution PASS is not hardware acceptance.

For a deliberate wrong-input control, repeat `guard-study` with the original
baseline in place of the candidate and a fresh directory. It must exit nonzero
with `GUARD_STUDY=FAIL`. Never edit a frozen baseline to force acceptance.

Build the separate firmware using the existing firmware cache:

```sh
(cd keyboard_keyboard/code && \
  CARGO_TARGET_DIR="$HOME/Documents/GitHub/keyboard-keyboard/keyboard_keyboard/code/target" \
  cargo build --release --locked \
    --manifest-path ../../circuits/experiments/power-fault-guard/firmware/Cargo.toml)
GUARD_ELF="$HOME/Documents/GitHub/keyboard-keyboard/keyboard_keyboard/code/target/thumbv7em-none-eabihf/release/keyboard-power-guard-firmware"
shasum -a 256 "$GUARD_ELF"
"$GUARD_LAB" guard-device circuits/baselines/keyboard-current.net \
  circuits/experiments/power-fault-guard/candidate.net circuits/fixtures/reference.fixture \
  "$GUARD_ELF" brownout circuits/results/verify-brownout-1 positive
```

Allow several minutes per ARM run. Repeat with `healthy`, `loss`, `startup-late`,
`startup-ramp`, `seed-brownout` and `chatter`, each with a fresh folder. Healthy
expects 28 balanced note pairs and all twelve initial pot controllers. Late/ramp
startups expect no notes and remain disarmed after rail recovery. Runtime faults
require explicit releases, no held notes, no NoteOn beyond the 3 ms observation
window, and complete panic within 170 ms. Reports separately count any NoteOn
inside that window. Compare raw MIDI/edge timestamps rather than only exit status.

The inherited memory map can emit a missing `Reset_Handler` ELF-entry warning.
Flash-vector startup is exercised by the ARM run; a successful link alone is not
the boot test. A new build may have a different ELF hash due to paths/toolchain;
identify it separately rather than relabeling it as the retained immutable ELF.

To reproduce the original-firmware negative, build `keyboard_keyboard/code`
normally with `cargo build --release --locked`, copy its `app` ELF to a fresh
result folder, then repeat `guard-device` with that ELF, `loss`, and final argument
`negative`. The report must be a functional **FAIL**, with late notes and no
emulator/startup errors; exit zero means the requested negative was reproduced.
The retained original ELF is SHA256
`e1416750d97d39086ea4f6fa9cb9987d1fda0965b09c844c19ca9b3d645ab3f1`.

`reset-after-loss` remains an intentionally exposed **unresolved warm-reset run**:
run it with `positive` to see its nonzero exit and flash startup error. Do not
change it to `negative` to turn an emulator/firmware startup error into acceptance.
Use full power cycling for the proposed operator recovery; the cold and runtime
segments are tested separately, not claimed as a physical continuous cycle.

For a drawing check:

```sh
kicad-cli sch export netlist --format kicadxml -o circuits/results/verify-guard.xml \
  circuits/experiments/power-fault-guard/power-guard-allocation.kicad_sch
"$GUARD_LAB" guard-check-schematic circuits/baselines/keyboard-current.net \
  circuits/experiments/power-fault-guard/candidate.net circuits/results/verify-guard.xml
```

Expected `SCHEMATIC_CONNECTIVITY=PASS` for the complete added allocation, including
rejection of extra/missing pins. The original-pin changes are separately listed
in `connection-changes.tsv`; this is not a PCB/footprint/ERC signoff. Mac KiCad CLI
may be `/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli`.

Hand-check the essentials: the two open-drain RESET outputs and PB5 share the
10 kΩ pullup; the sense rails remain separate; the original 4.7 nF filters remain.
`510 bytes × 10 bits / 31250 = 0.1632 s` gives the 90-pitch panic wire budget.
For startup, compare `3.07 × 1.015 × 1.025 ≈ 3.194 V` with `3.3 × 0.96 = 3.168 V`.
That mismatch is retained as a physical-design blocker, not hidden by the tests.

The next physical checks require supervisor/rail scope captures, actual source
tolerance, MCU reference/clock behavior during dips, analog loading, receiver
panic response and released-key calibration. Ultra-short glitches, deep Seed
collapse and total power loss remain outside the qualified virtual envelope.
