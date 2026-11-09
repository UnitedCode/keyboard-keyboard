/// Semantic performance events, shared by DIN and USB. Current sensors still
/// produce 7-bit velocity/CC and 14-bit pitch; wider packets add no precision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    PitchBend {
        channel: u8,
        value: u16,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    PitchBendRange {
        channel: u8,
        semitones: u8,
    },
}
impl Message {
    pub fn midi1(self) -> heapless::Vec<u8, 18> {
        let mut data = heapless::Vec::new();
        let bytes = match self {
            Self::NoteOn {
                channel,
                note,
                velocity,
            } => [0x90 | (channel & 15), note & 127, velocity & 127],
            Self::NoteOff {
                channel,
                note,
                velocity,
            } => [0x80 | (channel & 15), note & 127, velocity & 127],
            Self::ControlChange {
                channel,
                controller,
                value,
            } => [0xb0 | (channel & 15), controller & 127, value & 127],
            Self::PitchBend { channel, value } => [
                0xe0 | (channel & 15),
                (value & 127) as u8,
                ((value >> 7) & 127) as u8,
            ],
            Self::ProgramChange { channel, program } => {
                data.extend_from_slice(&[0xc0 | (channel & 15), program & 127])
                    .ok();
                return data;
            }
            Self::PitchBendRange { channel, semitones } => {
                for (cc, value) in [
                    (101, 0),
                    (100, 0),
                    (6, semitones & 127),
                    (38, 0),
                    (101, 127),
                    (100, 127),
                ] {
                    data.extend_from_slice(&[0xb0 | (channel & 15), cc, value])
                        .ok();
                }
                return data;
            }
        };
        data.extend_from_slice(&bytes).ok();
        data
    }
    pub fn ump2(self) -> [u32; 2] {
        match self {
            Self::NoteOn {
                channel,
                note,
                velocity,
            } if velocity & 127 == 0 => Self::NoteOff {
                channel,
                note,
                velocity: 0,
            }
            .ump2(),
            Self::NoteOn {
                channel,
                note,
                velocity,
            } => [
                0x40900000 | ((channel as u32 & 15) << 16) | ((note as u32 & 127) << 8),
                scale(velocity as u32 & 127, 7, 16) << 16,
            ],
            Self::NoteOff {
                channel,
                note,
                velocity,
            } => [
                0x40800000 | ((channel as u32 & 15) << 16) | ((note as u32 & 127) << 8),
                scale(velocity as u32 & 127, 7, 16) << 16,
            ],
            Self::ControlChange {
                channel,
                controller,
                value,
            } => [
                0x40b00000 | ((channel as u32 & 15) << 16) | ((controller as u32 & 127) << 8),
                scale(value as u32 & 127, 7, 32),
            ],
            Self::PitchBend { channel, value } => [
                0x40e00000 | ((channel as u32 & 15) << 16),
                scale(value as u32 & 16383, 14, 32),
            ],
            Self::ProgramChange { channel, program } => [
                0x40c00000 | ((channel as u32 & 15) << 16),
                (program as u32 & 127) << 24,
            ],
            Self::PitchBendRange { channel, semitones } => [
                0x40200000 | ((channel as u32 & 15) << 16),
                scale((semitones as u32 & 127) << 7, 14, 32),
            ],
        }
    }
}
// UMP Appendix D: align MSBs and repeat the lower source_bits-1 bits.
fn scale(value: u32, source_bits: u32, destination_bits: u32) -> u32 {
    let mut result = value << (destination_bits - source_bits);
    if value <= (1 << (source_bits - 1)) {
        return result;
    }

    let repeat_bits = source_bits - 1;
    let repeat = value & ((1 << repeat_bits) - 1);
    let mut left = destination_bits - source_bits;
    while left > 0 {
        let take = left.min(repeat_bits);
        left -= take;
        result |= (repeat >> (repeat_bits - take)) << left;
    }
    result
}
