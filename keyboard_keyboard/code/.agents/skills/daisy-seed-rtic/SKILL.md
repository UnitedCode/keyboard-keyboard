---
name: daisy-seed-rtic
description: Use when writing or modifying firmware for the Electrosmith Daisy Seed running RTIC v1. Covers GPIO, ADC, timers, audio, MIDI UART, I2C/display, software tasks, shared/local resources, and priority design for the STM32H750V + libdaisy-rust + stm32h7xx-hal stack.
---

# Daisy Seed RTIC Firmware Patterns

Reference projects (read-only standards):
- `synth-phone-e-v2-rust/` — fuller example with display, MIDI RX, button matrix, audio DSP
- `keyboard-keyboard/keyboard_keyboard/code/` — active keyboard project being built

---

## File Layout

```
src/
  main.rs          # RTIC app lives here (or split into modules)
  handler.rs       # Heavy task logic extracted for readability
  constants.rs     # SAMPLE_RATE, BLOCK_SIZE, etc.
  types.rs         # Type aliases (e.g. LcdDisplay)
  midi/            # MidiReceiver, MidiEvent, VoiceManager
  input/           # Button matrix, debounce helpers
  display/         # Screen drawing functions
Cargo.toml
memory.x
Embed.toml
```

---

## Cargo.toml essentials

```toml
[package]
edition = "2021"
name = "app"

[[bin]]
name = "app"
bench = false
doctest = false
test = false

[dependencies]
cortex-m-rtic = "1.1.4"
cortex-m = { version = "0.7.7", features = ["critical-section-single-core"] }
stm32h7xx-hal = { version = "0.16.0", features = ["stm32h750v", "rt"] }
libdaisy = { git = "https://github.com/nathansbradshaw/libdaisy-rust.git", features = ["log-rtt"] }
heapless = "0.8.0"
log = "0.4.21"
rtt-target = "0.5.0"
panic-rtt-target = "0.1.3"

[profile.dev]
codegen-units = 1
debug = true
lto = true
opt-level = "s"

[profile.release]
codegen-units = 1
debug = true
lto = true
opt-level = "s"
```

---

## App skeleton

```rust
#![no_std]
#![no_main]
#![deny(unsafe_code)]

mod rtic_app {
    #[rtic::app(
        device = stm32h7xx_hal::stm32,
        peripherals = true,
        dispatchers = [DMA1_STR2, DMA1_STR3, DMA1_STR4]  // one per software task
    )]
    mod app {
        use libdaisy::{audio, gpio::*, logger, system};
        use libdaisy::prelude::{Input, Output, PushPull};
        use stm32h7xx_hal::{stm32, time::MilliSeconds, time::U32Ext, timer};

        #[shared]
        struct Shared { /* fields accessed by multiple tasks */ }

        #[local]
        struct Local { /* fields owned by a single task */ }

        #[init]
        fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
            logger::init();
            let mut core = ctx.core;
            let device = ctx.device;
            let ccdr = system::System::init_clocks(device.PWR, device.RCC, &device.SYSCFG);
            let mut system = libdaisy::system_init!(core, device, ccdr, BLOCK_SIZE);
            // ... configure pins, peripherals ...
            (Shared { .. }, Local { .. }, init::Monotonics())
        }

        #[idle]
        fn idle(_ctx: idle::Context) -> ! {
            loop { cortex_m::asm::nop(); }
        }
    }
}
```

---

## GPIO

### Pin naming

`DaisyN` where N = the Daisy D-number from the Electrosmith pinout.
Physical pad number == Daisy D-number for pads 1–15 and 22–37.

| Physical pad | Daisy name | Alt uses |
|:---:|:---:|---|
| 1  | Daisy0  | USB_ID |
| 2  | Daisy1  | D1 / SD_DATA_3 |
| 7  | Daisy6  | D6 / SD_CLK |
| 8  | Daisy7  | D7 / SPI1_CS |
| 11 | Daisy10 | SPI1_MOSI |
| 12 | Daisy11 | I2C1_SCL |
| 13 | Daisy12 | I2C1_SDA |
| 14 | Daisy13 | USART1_TX |
| 15 | Daisy14 | USART1_RX |
| 22 | Daisy15 | A0 / ADC_0 |
| 23 | Daisy16 | A1 / ADC_1 |
| 29 | Daisy22 | A7 (DAC_OUT2, usable as ADC) |
| 31 | Daisy24 | A9 (SAI2_MCLK, usable as ADC) |

### Taking a pin

```rust
// Output
let mut led = system.gpio.daisy6.take().expect("daisy6").into_push_pull_output();

// Digital input (pull-up)
let btn = system.gpio.daisy1.take().expect("daisy1").into_pull_up_input();

// Floating input (for encoder)
let enc = system.gpio.daisy3.take().expect("daisy3").into_floating_input();

// Analog (ADC)
let adc_pin = system.gpio.daisy15.take().expect("daisy15").into_analog();

// Alternate function — USART1 = AF7, I2C1 = AF4
let tx = system.gpio.daisy13.take().expect("daisy13").into_alternate::<7>();

// I2C needs open-drain + pull-up
let sda = system.gpio.daisy12.take().expect("daisy12")
    .into_alternate::<4>()
    .internal_pull_up(true)
    .set_open_drain();
```

### Type annotations in structs

```rust
struct Local {
    led:     Daisy6<Output<PushPull>>,
    btn:     Daisy1<Input>,
    adc_pin: Daisy15<Analog>,
    s0:      Daisy7<Output<PushPull>>,
}
```

### Active-low LEDs

LEDs on this board are active-low (cathode → GPIO, anode → 3V3 via resistor).
`set_low()` = on, `set_high()` = off. Both are infallible; no `.unwrap()` needed.

---

## ADC

```rust
use stm32h7xx_hal::adc::{self, Adc, AdcSampleTime, Resolution};

let mut adc = system.adc1.enable();
adc.set_resolution(Resolution::TwelveBit);
adc.set_sample_time(AdcSampleTime::T_16);

// Reading (in task)
let value: u16 = adc.read(&mut pin).unwrap_or(0) as u16;
```

Allow a short `cortex_m::asm::delay(480 * 200)` after mux channel switch before reading.

---

## Timer (TIM2 @ 1 kHz)

```rust
use stm32h7xx_hal::{time::MilliSeconds, time::U32Ext, timer};

let mut timer2 = stm32h7xx_hal::timer::TimerExt::timer(
    device.TIM2,
    MilliSeconds::from_ticks(1).into_rate(),
    ccdr.peripheral.TIM2,
    &ccdr.clocks,
);
timer2.listen(timer::Event::TimeOut);
```

In the handler: `ctx.local.timer2.clear_irq();`

---

## Audio handler

Bind to `DMA1_STR1`. Use `BLOCK_SIZE` constant (typically 2–128).

```rust
#[task(binds = DMA1_STR1, local = [audio, buffer], shared = [...], priority = 8)]
fn audio_handler(mut ctx: audio_handler::Context) {
    if ctx.local.audio.get_stereo(ctx.local.buffer) {
        for (left, right) in &ctx.local.buffer.as_slice()[..BLOCK_SIZE] {
            // process samples
            if ctx.local.audio.push_stereo((out_l, out_r)).is_err() {
                log::warn!("audio push failed");
            }
        }
    }
}
```

Pass-through (no processing):
```rust
ctx.local.audio.for_each(|left, right| (left, right));
```

---

## MIDI UART (USART1, 31250 baud)

### TX (MidiSender)

See `src/midi_sender.rs`. Sends Note On/Off, CC, Pitch Bend over blocking UART.

```rust
use stm32h7xx_hal::serial::{config::Config as SerialConfig, SerialExt};

let tx_pin = system.gpio.daisy13.take().expect("daisy13").into_alternate::<7>();
let rx_pin = system.gpio.daisy14.take().expect("daisy14").into_alternate::<7>();
let mut cfg = SerialConfig::default();
cfg.baudrate = 31_250_u32.bps();
let serial = device.USART1.serial((tx_pin, rx_pin), cfg,
    ccdr.peripheral.USART1, &ccdr.clocks).unwrap();
let (tx, _rx) = serial.split();
```

### RX interrupt (MidiReceiver)

```rust
let (_tx, mut rx) = serial.split();
rx.listen();  // enable RXNE interrupt
// bind task to USART1
```

```rust
#[task(binds = USART1, local = [midi_receiver], shared = [midi_events], priority = 1)]
fn usart1_interrupt(ctx: usart1_interrupt::Context) {
    for _ in 0..8 {  // cap events per interrupt
        match ctx.local.midi_receiver.try_read() {
            Ok(Some(event)) => {
                ctx.shared.midi_events.lock(|q| { q.enqueue(event).ok(); });
            }
            _ => break,
        }
    }
}
```

---

## I2C + SSD1306 display

```rust
use stm32h7xx_hal::i2c::{I2c, I2cExt};
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};

let scl = system.gpio.daisy11.take().expect("daisy11")
    .into_alternate::<4>().internal_pull_up(true).set_open_drain();
let sda = system.gpio.daisy12.take().expect("daisy12")
    .into_alternate::<4>().internal_pull_up(true).set_open_drain();

let i2c = device.I2C1.i2c((scl, sda), 100_u32.kHz(),
    ccdr.peripheral.I2C1, &ccdr.clocks);

let interface = I2CDisplayInterface::new_custom_address(i2c, 0x3C);
let mut display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
    .into_buffered_graphics_mode();
display.init().expect("display init failed");
display.clear();
// draw with embedded-graphics, then:
display.flush().ok();
```

Type alias:
```rust
type LcdDisplay = Ssd1306<
    I2CInterface<I2c<stm32::I2C1>>,
    DisplaySize128x32,
    BufferedGraphicsMode<DisplaySize128x32>,
>;
```

---

## RTIC task patterns

### Hardware task (interrupt-bound)

```rust
#[task(
    binds = TIM2,
    local  = [timer2, ...],
    shared = [tick_ms, ...],
    priority = 3
)]
fn timer_handler(mut ctx: timer_handler::Context) {
    ctx.local.timer2.clear_irq();
    // ...
}
```

### Software task (spawned)

Requires a free interrupt in `dispatchers = [...]` in the `#[rtic::app]` attribute.

```rust
#[task(shared = [...], local = [...], priority = 1, capacity = 4)]
fn my_task(mut ctx: my_task::Context) { ... }

// Spawn from another task:
my_task::spawn().ok();
// With args:
my_task::spawn(arg1, arg2).ok();
```

### Shared resource locking

```rust
// Single lock
ctx.shared.tick_ms.lock(|t| { *t += 1; });

// Multi-lock (avoids deadlock — always lock in declaration order)
(ctx.shared.field_a, ctx.shared.field_b).lock(|a, b| {
    // use a and b together
});
```

### Dispatchers

One free interrupt per concurrent software task. Use DMA stream interrupts not otherwise claimed:

```
dispatchers = [DMA1_STR2, DMA1_STR3, DMA1_STR4, DMA1_STR5]
```

---

## Priority guidelines

| Task | Binding | Priority |
|------|---------|----------|
| Audio | `DMA1_STR1` | 8 |
| FFT/DSP | software | 7 |
| Interface (TIM2) | `TIM2` | 3 |
| MIDI batch | software | 3 |
| Display update | software | 2 |
| Event processing | software | 1 |
| USART1 MIDI RX | `USART1` | 1 |

Higher number = higher priority. Audio must be highest to prevent underruns.
Never block in high-priority tasks; defer heavy work to lower-priority software tasks.

---

## Inter-task queues

```rust
// In Shared:
event_queue: heapless::spsc::Queue<MyEvent, 16>,

// Enqueue (from ISR/task):
ctx.shared.event_queue.lock(|q| { q.enqueue(event).ok(); });

// Dequeue (in consumer task):
ctx.shared.event_queue.lock(|q| {
    while let Some(event) = q.dequeue() { ... }
});
```

---

## hid::Switch (debounced button)

```rust
use libdaisy::hid;

let pin = system.gpio.daisy2.take().expect("daisy2").into_pull_up_input();
let mut btn = hid::Switch::new(pin, hid::SwitchType::PullUp);
btn.set_double_thresh(Some(100));   // ms for double-click
btn.set_held_thresh(Some(150));      // ms for hold

// In task (call every ~1ms):
btn.update();
if btn.is_falling() { /* single press */ }
if btn.is_double()  { /* double press */ }
if btn.is_high()    { /* held down    */ }
```

---

## Common pitfalls

- **Forgot `clear_irq()`** in timer handler → infinite interrupt storm
- **Taking same pin twice** → `expect()` panics at runtime; each `daisyN` can only be taken once
- **Shared resource without lock** → won't compile; RTIC enforces this
- **Software task with no dispatcher** → won't compile; add to `dispatchers = [...]`
- **Blocking in high-priority task** → causes audio underruns; spawn a lower-priority task instead
- **I2C without open-drain** → bus contention; always `.set_open_drain()` for I2C pins
- **ADC read before settling** → add `cortex_m::asm::delay(480 * N)` after mux switch
- **`opt-level = "s"`** required in both dev and release for timing-sensitive DSP on Cortex-M7
