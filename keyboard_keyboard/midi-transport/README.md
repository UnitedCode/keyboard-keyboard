# Keyboard MIDI transport experiment

Heapless, `no_std` USB MIDI 1.0/2.0 class and independent DIN/USB output queues.
The firmware uses this crate directly; the host fixture runs the same USB class
through `usb-device` using simulated control and bulk transfers.

See [USB MIDI branch guide](../code/USB_MIDI.md) for the screen controls, wiring,
protocol scope, commands, test identity restrictions and remaining device checks.

The simulated bus does not model USB electrical signaling, host OS behavior or
STM32 endpoint registers. The screen preview includes the firmware's actual
rendering and settings sources; it does not emulate OLED I2C timing.
