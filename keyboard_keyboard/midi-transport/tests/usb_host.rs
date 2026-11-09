mod support;
use keyboard_midi_transport::UsbMidi;
use usb_device::{bus::UsbBusAllocator, prelude::*};

#[test]
fn computer_can_enumerate_legacy_and_ump_alternates() {
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 0x0001))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    let descriptor = host.control_in(&mut dev, &mut midi, [0x80, 6, 0, 2, 0, 0, 255, 0]);
    assert_eq!(descriptor[4], 2, "audio control + MIDI streaming");
    let descriptors: Vec<_> = support::descriptors(&descriptor).collect();
    assert!(descriptors
        .iter()
        .any(|d| d[1] == 4 && d[2..8] == [1, 0, 2, 1, 3, 0]));
    assert!(descriptors
        .iter()
        .any(|d| d[1] == 4 && d[2..8] == [1, 1, 2, 1, 3, 0]));
    assert!(descriptors.iter().any(|d| *d == [7, 0x24, 1, 0, 2, 7, 0]));
    let blocks = host.control_in(&mut dev, &mut midi, [0x81, 6, 1, 0x26, 1, 0, 255, 0]);
    assert_eq!(&blocks[..5], &[5, 0x26, 1, 18, 0]);
    assert_eq!(
        &blocks[5..],
        &[13, 0x26, 2, 1, 0, 0, 1, 4, 0x11, 0, 0, 0, 0]
    );
}

#[test]
fn computer_receives_legacy_notes_then_can_select_midi2() {
    use keyboard_midi_transport::Message;
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 0x0001))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    midi.write_message(Message::NoteOn {
        channel: 0,
        note: 60,
        velocity: 127,
    })
    .unwrap();
    assert_eq!(host.take(0x81), Some(vec![9, 0x90, 60, 127]));
    dev.poll(&mut [&mut midi]);
    host.control_out(&mut dev, &mut midi, [1, 11, 1, 0, 1, 0, 0, 0]);
    midi.write_message(Message::NoteOn {
        channel: 0,
        note: 60,
        velocity: 127,
    })
    .unwrap();
    assert_eq!(
        host.take(0x81),
        Some(vec![0, 60, 0x90, 0x40, 0, 0, 255, 255])
    );
    dev.poll(&mut [&mut midi]);
    // Stream Configuration Request: protocol MIDI 1.0 within the UMP transport.
    host.send(1, &[0, 1, 5, 0xf0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    dev.poll(&mut [&mut midi]);
    assert_eq!(
        host.take(0x81),
        Some(vec![0, 1, 6, 0xf0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    );
    dev.poll(&mut [&mut midi]);
    midi.write_message(Message::NoteOn {
        channel: 0,
        note: 60,
        velocity: 127,
    })
    .unwrap();
    assert_eq!(host.take(0x81), Some(vec![127, 60, 0x90, 0x20]));
}

#[test]
fn midi2_velocity_matches_specification_min_center_max_examples() {
    use keyboard_midi_transport::Message;
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 1))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [1, 11, 1, 0, 1, 0, 0, 0]);
    // UMP v1.1.1 Appendix D's worked examples, not calculated expectations.
    for (velocity, expected) in [
        (10, [0, 0, 0, 0x14]),
        (64, [0, 0, 0, 0x80]),
        (87, [0, 0, 0xba, 0xae]),
        (127, [0, 0, 0xff, 0xff]),
    ] {
        midi.write_message(Message::NoteOn {
            channel: 0,
            note: 60,
            velocity,
        })
        .unwrap();
        let packet = host.take(0x81).unwrap();
        assert_eq!(&packet[4..], &expected);
        dev.poll(&mut [&mut midi]);
    }
}

#[test]
fn computer_discovers_identity_capabilities_and_keyboard_function() {
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 1))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [1, 11, 1, 0, 1, 0, 0, 0]);
    // Endpoint discovery, requesting all five defined notifications.
    host.send(1, &[1, 1, 0, 0xf0, 31, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    dev.poll(&mut [&mut midi]);
    let mut packets = Vec::new();
    for _ in 0..16 {
        if let Some(p) = host.take(0x81) {
            packets.push(p);
            dev.poll(&mut [&mut midi]);
        } else {
            break;
        }
    }
    assert!(packets.contains(&vec![1, 1, 1, 0xf0, 0, 3, 0, 0x81, 0, 0, 0, 0, 0, 0, 0, 0]));
    assert!(packets
        .iter()
        .any(|p| p[2] == 2 && p[7] == 0 && p[6] == 0x7d));
    assert!(packets.iter().any(|p| p[2] == 3), "endpoint name");
    assert!(packets.iter().any(|p| p[2] == 4), "stable instance ID");
    assert!(packets.contains(&vec![0, 2, 6, 0xf0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
    host.send(
        1,
        &[3, 0xff, 0x10, 0xf0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
    dev.poll(&mut [&mut midi]);
    assert_eq!(
        host.take(0x81),
        Some(vec![
            0x23, 0x80, 0x11, 0xf0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ])
    );
    dev.poll(&mut [&mut midi]);
    assert_eq!(
        host.take(0x81),
        Some(vec![
            b'K', 0, 0x12, 0xf0, b'o', b'b', b'y', b'e', 0, b'd', b'r', b'a', 0, 0, 0, 0
        ])
    );
}

#[test]
fn blocked_usb_does_not_block_din_and_route_changes_silence_old_destination() {
    use keyboard_midi_transport::{Message, OutputRoute, Outputs};
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 1))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    let mut outputs = Outputs::new();
    for _ in 0..16 {
        outputs.service_usb(&mut midi, true);
        host.take(0x81);
        dev.poll(&mut [&mut midi]);
    }
    host.blocked(true);
    outputs.send(Message::NoteOn {
        channel: 0,
        note: 60,
        velocity: 127,
    });
    outputs.service_usb(&mut midi, true);
    let mut din = Vec::new();
    while let Some(byte) = outputs.peek_din_byte() {
        din.push(byte);
        outputs.advance_din();
    }
    assert_eq!(din, [0x90, 60, 127]);
    host.blocked(false);
    outputs.service_usb(&mut midi, true);
    assert_eq!(host.take(0x81), Some(vec![9, 0x90, 60, 127]));
    dev.poll(&mut [&mut midi]);
    outputs.set_route(OutputRoute::Usb);
    let mut panic = Vec::new();
    while let Some(byte) = outputs.peek_din_byte() {
        panic.push(byte);
        outputs.advance_din();
    }
    assert_eq!(panic.len(), 48);
    assert_eq!(&panic[..3], &[0xb0, 123, 0]);
    assert_eq!(&panic[45..], &[0xbf, 123, 0]);
    outputs.send(Message::NoteOff {
        channel: 0,
        note: 60,
        velocity: 0,
    });
    assert_eq!(outputs.peek_din_byte(), None);
    for _ in 0..16 {
        outputs.service_usb(&mut midi, true);
        host.take(0x81);
        dev.poll(&mut [&mut midi]);
    }
    outputs.service_usb(&mut midi, true);
    assert_eq!(host.take(0x81), Some(vec![8, 0x80, 60, 0]));
}

#[test]
fn midi2_wire_messages_preserve_notes_controls_pitch_program_and_bend_range() {
    use keyboard_midi_transport::Message::*;
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 1))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [1, 11, 1, 0, 1, 0, 0, 0]);
    for (message, expected) in [
        (
            NoteOn {
                channel: 0,
                note: 60,
                velocity: 0,
            },
            [0, 60, 0x80, 0x40, 0, 0, 0, 0],
        ),
        (
            NoteOff {
                channel: 9,
                note: 36,
                velocity: 0,
            },
            [0, 36, 0x89, 0x40, 0, 0, 0, 0],
        ),
        (
            ControlChange {
                channel: 0,
                controller: 7,
                value: 64,
            },
            [0, 7, 0xb0, 0x40, 0, 0, 0, 0x80],
        ),
        (
            ControlChange {
                channel: 0,
                controller: 7,
                value: 127,
            },
            [0, 7, 0xb0, 0x40, 255, 255, 255, 255],
        ),
        (
            PitchBend {
                channel: 0,
                value: 8192,
            },
            [0, 0, 0xe0, 0x40, 0, 0, 0, 0x80],
        ),
        (
            ProgramChange {
                channel: 2,
                program: 5,
            },
            [0, 0, 0xc2, 0x40, 0, 0, 0, 5],
        ),
        (
            PitchBendRange {
                channel: 0,
                semitones: 2,
            },
            [0, 0, 0x20, 0x40, 0, 0, 0, 4],
        ),
    ] {
        midi.write_message(message).unwrap();
        assert_eq!(host.take(0x81), Some(expected.to_vec()));
        dev.poll(&mut [&mut midi]);
    }
}

#[test]
fn host_can_fragment_discovery_and_reserved_protocol_does_not_change_output() {
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 1))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [1, 11, 1, 0, 1, 0, 0, 0]);
    host.send(1, &[1, 1, 0, 0xf0]);
    dev.poll(&mut [&mut midi]);
    assert_eq!(host.take(0x81), None);
    host.send(1, &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    dev.poll(&mut [&mut midi]);
    assert_eq!(
        host.take(0x81),
        Some(vec![1, 1, 1, 0xf0, 0, 3, 0, 0x81, 0, 0, 0, 0, 0, 0, 0, 0])
    );
    dev.poll(&mut [&mut midi]);
    host.send(1, &[3, 3, 5, 0xf0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    dev.poll(&mut [&mut midi]);
    assert_eq!(
        host.take(0x81),
        Some(vec![0, 2, 6, 0xf0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    );
}

#[test]
fn saturated_din_recovers_without_losing_usb_performance() {
    use keyboard_midi_transport::{Message, Outputs};
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 1))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    let mut outputs = Outputs::new();
    for _ in 0..16 {
        outputs.service_usb(&mut midi, true);
        host.take(0x81);
        dev.poll(&mut [&mut midi]);
    }
    for _ in 0..600 {
        outputs.send(Message::ControlChange {
            channel: 0,
            controller: 1,
            value: 64,
        });
        outputs.service_usb(&mut midi, true);
        assert_eq!(host.take(0x81), Some(vec![11, 0xb0, 1, 64]));
        dev.poll(&mut [&mut midi]);
    }
    assert!(outputs.overflows > 0);
    let mut din = Vec::new();
    while let Some(byte) = outputs.peek_din_byte() {
        din.push(byte);
        outputs.advance_din();
    }
    for channel in 0..16 {
        assert_eq!(
            &din[channel * 3..channel * 3 + 3],
            &[0xb0 | channel as u8, 123, 0]
        );
    }
}

#[test]
fn usb_reconnect_drops_stale_notes_and_starts_with_all_notes_off() {
    use keyboard_midi_transport::{Message, Outputs};
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 1))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    let mut outputs = Outputs::new();
    for _ in 0..16 {
        outputs.service_usb(&mut midi, true);
        host.take(0x81);
        dev.poll(&mut [&mut midi]);
    }
    outputs.send(Message::NoteOn {
        channel: 0,
        note: 60,
        velocity: 127,
    });
    host.reset(&mut dev, &mut midi);
    outputs.service_usb(&mut midi, false);
    outputs.send(Message::NoteOn {
        channel: 0,
        note: 64,
        velocity: 127,
    });
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    for channel in 0..16 {
        outputs.service_usb(&mut midi, true);
        assert_eq!(host.take(0x81), Some(vec![11, 0xb0 | channel, 123, 0]));
        dev.poll(&mut [&mut midi]);
    }
    outputs.service_usb(&mut midi, true);
    assert_eq!(host.take(0x81), None);
}

#[test]
fn host_cannot_select_ump_if_endpoint_reset_fails() {
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567").with_endpoint_reset(|_, _| false);
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 1))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.setup(&mut dev, &mut midi, [1, 11, 1, 0, 1, 0, 0, 0]);
    assert!(host.stalled(0x80));
    assert_eq!(midi.mode_label(), "M1");
}

#[test]
fn malformed_usb_packet_does_not_poison_next_discovery() {
    let host = support::Host::default();
    let bus = UsbBusAllocator::new(host.bus());
    let mut midi = UsbMidi::new(&bus, "0123456789ABCDEF01234567");
    let mut dev = UsbDeviceBuilder::new(&bus, UsbVidPid(0x1209, 1))
        .max_packet_size_0(64)
        .unwrap()
        .build();
    host.reset(&mut dev, &mut midi);
    host.control_out(&mut dev, &mut midi, [0, 5, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [0, 9, 1, 0, 0, 0, 0, 0]);
    host.control_out(&mut dev, &mut midi, [1, 11, 1, 0, 1, 0, 0, 0]);
    host.send(1, &[0, 1, 5, 0xf0, 0]);
    dev.poll(&mut [&mut midi]);
    host.send(1, &[1, 1, 0, 0xf0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    dev.poll(&mut [&mut midi]);
    assert_eq!(
        host.take(0x81),
        Some(vec![1, 1, 1, 0xf0, 0, 3, 0, 0x81, 0, 0, 0, 0, 0, 0, 0, 0])
    );
}
