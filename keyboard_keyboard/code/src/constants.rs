pub const BLOCK_SIZE: usize = 128;

pub const NUM_SWITCHES: usize = 100;
pub const NUM_MUXES: usize = 13;
pub const NUM_ADC_PINS: usize = 9;
pub const MUX_CHANNELS: usize = 8;

// ── Thresholds ────────────────────────────────────────────────────────────────
pub const FIRST_DELTA: u16 = 150;
pub const SECOND_DELTA: u16 = 250;
pub const RELEASE_DELTA: u16 = 100;
pub const DEBOUNCE_TICKS: u8 = 3;

pub const FILTER_SIZE: usize = 4;
pub const FILTER_SHIFT: u32 = 2;

pub const VELOCITY_WINDOW_MS: u32 = 80;
pub const CALIBRATION_SAMPLES: usize = 64;

pub const DIAG_LOGGING: bool = false; // set true to see raw ADC / calibration logs
pub const LOG_INTERVAL_MS: u32 = 500;
pub const LOG_SWITCH: usize = 0; // HE1 — first switch

// ── Settings buttons ──────────────────────────────────────────────────────────
pub const SETTINGS_CHAN1: usize = 72; // HE73 → melody MIDI ch 1
pub const SETTINGS_CHAN2: usize = 75; // HE76 → melody MIDI ch 2

// ── Drum pads (switches 81–100, MIDI ch 10) ───────────────────────────────────
pub const DRUM_CHANNEL: u8 = 9; // 0-indexed
pub const DRUM_SWITCH_START: usize = 80;
#[rustfmt::skip]
pub const DRUM_NOTE: [u8; 20] = [
    36, 37, 38, 39, 40, 41, 42, 43, 44, 45, // GM Bass Drum → Pedal Hi-Hat
    46, 47, 48, 49, 50, 51, 52, 53, 54, 55, // GM Open Hi-Hat → Splash Cymbal
];

// ── Pitch bend (left/right arrow keys) ───────────────────────────────────────
pub const PITCH_BEND_DOWN: usize = 77; // HE78 → pitch down
pub const PITCH_BEND_UP: usize = 79; // HE80 → pitch up
pub const PITCH_BEND_MAX_DELTA: u16 = 400;
pub const PITCH_BEND_HYSTERESIS: u16 = 32;
pub const PITCH_BEND_INTERVAL_MS: u32 = 5;

// ── Vibrato (up/down arrow keys) → CC1 ───────────────────────────────────────
pub const VIBRATO_A: usize = 76; // HE77 → vibrato depth
pub const VIBRATO_B: usize = 78; // HE79 → vibrato depth
pub const VIBRATO_MAX_DELTA: u16 = 300;
pub const VIBRATO_DEAD_ZONE: u16 = 30; // noise floor below which output = 0
pub const VIBRATO_HYSTERESIS: u8 = 2;
pub const VIBRATO_INTERVAL_MS: u32 = 10;

// ── Potentiometers ────────────────────────────────────────────────────────────
pub const NUM_POTS: usize = 12;
pub const POT_SCAN_MS: u32 = 10;
pub const POT_CC_HYSTERESIS: u8 = 2;
// (decoder_idx, mux_channel, CC_number)
// decoder_idx 4 = AM14 (Y4), 5 = AM15 (Y5) — both read via Daisy28 / A11
#[rustfmt::skip]
pub const POT_MAP: [(u8, u8, u8); NUM_POTS] = [
    (4, 4, 20), // RV1  AM14 X4 → CC20
    (4, 6, 21), // RV2  AM14 X6 → CC21
    (4, 7, 22), // RV3  AM14 X7 → CC22
    (4, 5, 23), // RV4  AM14 X5 → CC23
    (4, 2, 24), // RV5  AM14 X2 → CC24
    (4, 1, 25), // RV6  AM14 X1 → CC25
    (4, 0, 26), // RV7  AM14 X0 → CC26
    (4, 3, 27), // RV8  AM14 X3 → CC27
    (5, 4, 28), // RV9  AM15 X4 → CC28
    (5, 6, 29), // RV10 AM15 X6 → CC29
    (5, 7, 30), // RV11 AM15 X7 → CC30
    (5, 5, 31), // RV12 AM15 X5 → CC31
];

// ── Switch map: (mux_index, channel) per switch index ────────────────────────
// mux_index: 0=AM1 … 8=AM9 (Daisy15–23 / A0–A8), 9–12=AM10–13 via U1 decoder
#[rustfmt::skip]
pub const SWITCH_MAP: [(u8, u8); NUM_SWITCHES] = [
    // AM1 (mux 0, Daisy15/A0) — HE1–HE8
    (0, 4), (0, 6), (0, 7), (0, 5), (0, 2), (0, 1), (0, 0), (0, 3),
    // AM2 (mux 1, Daisy16/A1) — HE9–HE14
    (1, 4), (1, 6), (1, 7), (1, 5), (1, 2), (1, 1),
    // AM3 (mux 2, Daisy17/A2) — HE15–HE22
    (2, 4), (2, 6), (2, 7), (2, 5), (2, 2), (2, 1), (2, 0), (2, 3),
    // AM4 (mux 3, Daisy18/A3) — HE23–HE27
    (3, 4), (3, 6), (3, 7), (3, 5), (3, 2),
    // AM2 continued — HE28–HE29
    (1, 3), (1, 0),
    // AM5 (mux 4, Daisy19/A4) — HE30–HE37
    (4, 4), (4, 6), (4, 7), (4, 5), (4, 2), (4, 1), (4, 0), (4, 3),
    // AM6 (mux 5, Daisy20/A5) — HE38–HE40
    (5, 4), (5, 6), (5, 7),
    // AM4 continued — HE41–HE43
    (3, 1), (3, 0), (3, 3),
    // AM7 (mux 6, Daisy21/A6) — HE44–HE51
    (6, 4), (6, 6), (6, 7), (6, 5), (6, 2), (6, 1), (6, 0), (6, 3),
    // AM8 (mux 7, Daisy22/A7) — HE52–HE53
    (7, 4), (7, 6),
    // AM6 continued — HE54–HE58
    (5, 5), (5, 2), (5, 1), (5, 0), (5, 3),
    // AM9 (mux 8, Daisy23/A8) — HE59–HE66
    (8, 4), (8, 6), (8, 7), (8, 5), (8, 2), (8, 1), (8, 0), (8, 3),
    // AM8 continued — HE67–HE70
    (7, 7), (7, 5), (7, 2), (7, 1),
    // AM10 (mux 9, decoder) — HE71–HE73
    (9, 4), (9, 6), (9, 7),
    // AM11 (mux 10, decoder) — HE74–HE80
    (10, 4), (10, 6), (10, 7), (10, 5), (10, 2), (10, 1), (10, 0),
    // AM10 continued — HE81–HE84
    (9, 5), (9, 2), (9, 1), (9, 0),
    // AM12 (mux 11, decoder) — HE85–HE92
    (11, 4), (11, 6), (11, 7), (11, 5), (11, 2), (11, 1), (11, 0), (11, 3),
    // AM13 (mux 12, decoder) — HE93–HE100
    (12, 4), (12, 6), (12, 7), (12, 5), (12, 2), (12, 1), (12, 0), (12, 3),
];

// ── Wicki-Hayden MIDI note layout ────────────────────────────────────────────
// Rows alternate between two whole-tone scales a perfect fourth apart.
// Row 1 → HE1–14, row 2 → HE15–29, row 3 → HE30–43,
// row 4 → HE44–58, row 5 → HE59–70. Switches 71+ unused (note 0).
pub const SWITCH_TO_NOTE: [u8; NUM_SWITCHES] = [
    // Row 1 — HE1–14  (base 78, whole-tone steps)
    78, 80, 82, 84, 86, 88, 90, 92, 94, 96, 98, 100, 102, 104,
    // Row 2 — HE15–29 (base 71)
    71, 73, 75, 77, 79, 81, 83, 85, 87, 89, 91, 93, 95, 97, 99,
    // Row 3 — HE30–43 (base 66)
    66, 68, 70, 72, 74, 76, 78, 80, 82, 84, 86, 88, 90, 92,
    // Row 4 — HE44–58 (base 59)
    59, 61, 63, 65, 67, 69, 71, 73, 75, 77, 79, 81, 83, 85, 87,
    // Row 5 — HE59–70 (base 54)
    54, 58, 60, 62, 64, 66, 68, 70, 72, 74, 76,
    80, // HE71 — skipped 56 and 78 (those keys don't exist on the board)
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

// ── HE sensor number per switch index (for log messages) ─────────────────────
#[rustfmt::skip]
pub const HE_NUM: [u8; NUM_SWITCHES] = [
     1,  2,  3,  4,  5,  6,  7,  8, // switch  0–7  : HE1–HE8   (AM1)
     9, 10, 11, 12, 13, 14,          // switch  8–13 : HE9–HE14  (AM2)
    15, 16, 17, 18, 19, 20, 21, 22,  // switch 14–21 : HE15–HE22 (AM3)
    23, 24, 25, 26, 27,              // switch 22–26 : HE23–HE27 (AM4)
    28, 29,                          // switch 27–28 : HE28–HE29 (AM2)
    30, 31, 32, 33, 34, 35, 36, 37, // switch 29–36 : HE30–HE37 (AM5)
    38, 39, 40,                      // switch 37–39 : HE38–HE40 (AM6)
    41, 42, 43,                      // switch 40–42 : HE41–HE43 (AM4)
    44, 45, 46, 47, 48, 49, 50, 51, // switch 43–50 : HE44–HE51 (AM7)
    52, 53,                          // switch 51–52 : HE52–HE53 (AM8)
    54, 55, 56, 57, 58,              // switch 53–57 : HE54–HE58 (AM6)
    59, 60, 61, 62, 63, 64, 65, 66, // switch 58–65 : HE59–HE66 (AM9)
    67, 68, 69, 70,                  // switch 66–69 : HE67–HE70 (AM8)
    71, 72, 73,                      // switch 70–72 : HE71–HE73 (AM10)
    74, 75, 76, 77, 78, 79, 80,      // switch 73–79 : HE74–HE80 (AM11)
    81, 82, 83, 84,                  // switch 80–83 : HE81–HE84 (AM10)
    85, 86, 87, 88, 89, 90, 91, 92, // switch 84–91 : HE85–HE92 (AM12)
    93, 94, 95, 96, 97, 98, 99, 100, // switch 92–99 : HE93–HE100 (AM13)
];
