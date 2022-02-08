//! SPC700 Sound Coprocessor handling types
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/spc700-reference>
//! - <https://emudev.de/q00-snes/spc700-the-audio-processor/>
//! - The first of the two official SNES documentation books

use crate::timing::Cycles;
use core::{cell::Cell, mem::take};
use save_state::{SaveStateDeserializer, SaveStateSerializer};
use save_state_macro::*;

pub const MEMORY_SIZE: usize = 64 * 1024;

static ROM: [u8; 64] = [
    0xCD, 0xEF, 0xBD, 0xE8, 0x00, 0xC6, 0x1D, 0xD0, 0xFC, 0x8F, 0xAA, 0xF4, 0x8F, 0xBB, 0xF5, 0x78,
    0xCC, 0xF4, 0xD0, 0xFB, 0x2F, 0x19, 0xEB, 0xF4, 0xD0, 0xFC, 0x7E, 0xF4, 0xD0, 0x0B, 0xE4, 0xF5,
    0xCB, 0xF4, 0xD7, 0x00, 0xFC, 0xD0, 0xF3, 0xAB, 0x01, 0x10, 0xEF, 0x7E, 0xF4, 0x10, 0xEB, 0xBA,
    0xF6, 0xDA, 0x00, 0xBA, 0xF4, 0xC4, 0xF4, 0xDD, 0x5D, 0xD0, 0xDB, 0x1F, 0x00, 0x00, 0xC0, 0xFF,
];

const GAUSS_INTERPOLATION_POINTS: [u16; 16 * 32] = [
    0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000,
    0x000, 0x000, 0x000, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001,
    0x001, 0x002, 0x002, 0x002, 0x002, 0x002, 0x002, 0x002, 0x003, 0x003, 0x003, 0x003, 0x003,
    0x004, 0x004, 0x004, 0x004, 0x004, 0x005, 0x005, 0x005, 0x005, 0x006, 0x006, 0x006, 0x006,
    0x007, 0x007, 0x007, 0x008, 0x008, 0x008, 0x009, 0x009, 0x009, 0x00A, 0x00A, 0x00A, 0x00B,
    0x00B, 0x00B, 0x00C, 0x00C, 0x00D, 0x00D, 0x00E, 0x00E, 0x00F, 0x00F, 0x00F, 0x010, 0x010,
    0x011, 0x011, 0x012, 0x013, 0x013, 0x014, 0x014, 0x015, 0x015, 0x016, 0x017, 0x017, 0x018,
    0x018, 0x019, 0x01A, 0x01B, 0x01B, 0x01C, 0x01D, 0x01D, 0x01E, 0x01F, 0x020, 0x020, 0x021,
    0x022, 0x023, 0x024, 0x024, 0x025, 0x026, 0x027, 0x028, 0x029, 0x02A, 0x02B, 0x02C, 0x02D,
    0x02E, 0x02F, 0x030, 0x031, 0x032, 0x033, 0x034, 0x035, 0x036, 0x037, 0x038, 0x03A, 0x03B,
    0x03C, 0x03D, 0x03E, 0x040, 0x041, 0x042, 0x043, 0x045, 0x046, 0x047, 0x049, 0x04A, 0x04C,
    0x04D, 0x04E, 0x050, 0x051, 0x053, 0x054, 0x056, 0x057, 0x059, 0x05A, 0x05C, 0x05E, 0x05F,
    0x061, 0x063, 0x064, 0x066, 0x068, 0x06A, 0x06B, 0x06D, 0x06F, 0x071, 0x073, 0x075, 0x076,
    0x078, 0x07A, 0x07C, 0x07E, 0x080, 0x082, 0x084, 0x086, 0x089, 0x08B, 0x08D, 0x08F, 0x091,
    0x093, 0x096, 0x098, 0x09A, 0x09C, 0x09F, 0x0A1, 0x0A3, 0x0A6, 0x0A8, 0x0AB, 0x0AD, 0x0AF,
    0x0B2, 0x0B4, 0x0B7, 0x0BA, 0x0BC, 0x0BF, 0x0C1, 0x0C4, 0x0C7, 0x0C9, 0x0CC, 0x0CF, 0x0D2,
    0x0D4, 0x0D7, 0x0DA, 0x0DD, 0x0E0, 0x0E3, 0x0E6, 0x0E9, 0x0EC, 0x0EF, 0x0F2, 0x0F5, 0x0F8,
    0x0FB, 0x0FE, 0x101, 0x104, 0x107, 0x10B, 0x10E, 0x111, 0x114, 0x118, 0x11B, 0x11E, 0x122,
    0x125, 0x129, 0x12C, 0x130, 0x133, 0x137, 0x13A, 0x13E, 0x141, 0x145, 0x148, 0x14C, 0x150,
    0x153, 0x157, 0x15B, 0x15F, 0x162, 0x166, 0x16A, 0x16E, 0x172, 0x176, 0x17A, 0x17D, 0x181,
    0x185, 0x189, 0x18D, 0x191, 0x195, 0x19A, 0x19E, 0x1A2, 0x1A6, 0x1AA, 0x1AE, 0x1B2, 0x1B7,
    0x1BB, 0x1BF, 0x1C3, 0x1C8, 0x1CC, 0x1D0, 0x1D5, 0x1D9, 0x1DD, 0x1E2, 0x1E6, 0x1EB, 0x1EF,
    0x1F3, 0x1F8, 0x1FC, 0x201, 0x205, 0x20A, 0x20F, 0x213, 0x218, 0x21C, 0x221, 0x226, 0x22A,
    0x22F, 0x233, 0x238, 0x23D, 0x241, 0x246, 0x24B, 0x250, 0x254, 0x259, 0x25E, 0x263, 0x267,
    0x26C, 0x271, 0x276, 0x27B, 0x280, 0x284, 0x289, 0x28E, 0x293, 0x298, 0x29D, 0x2A2, 0x2A6,
    0x2AB, 0x2B0, 0x2B5, 0x2BA, 0x2BF, 0x2C4, 0x2C9, 0x2CE, 0x2D3, 0x2D8, 0x2DC, 0x2E1, 0x2E6,
    0x2EB, 0x2F0, 0x2F5, 0x2FA, 0x2FF, 0x304, 0x309, 0x30E, 0x313, 0x318, 0x31D, 0x322, 0x326,
    0x32B, 0x330, 0x335, 0x33A, 0x33F, 0x344, 0x349, 0x34E, 0x353, 0x357, 0x35C, 0x361, 0x366,
    0x36B, 0x370, 0x374, 0x379, 0x37E, 0x383, 0x388, 0x38C, 0x391, 0x396, 0x39B, 0x39F, 0x3A4,
    0x3A9, 0x3AD, 0x3B2, 0x3B7, 0x3BB, 0x3C0, 0x3C5, 0x3C9, 0x3CE, 0x3D2, 0x3D7, 0x3DC, 0x3E0,
    0x3E5, 0x3E9, 0x3ED, 0x3F2, 0x3F6, 0x3FB, 0x3FF, 0x403, 0x408, 0x40C, 0x410, 0x415, 0x419,
    0x41D, 0x421, 0x425, 0x42A, 0x42E, 0x432, 0x436, 0x43A, 0x43E, 0x442, 0x446, 0x44A, 0x44E,
    0x452, 0x455, 0x459, 0x45D, 0x461, 0x465, 0x468, 0x46C, 0x470, 0x473, 0x477, 0x47A, 0x47E,
    0x481, 0x485, 0x488, 0x48C, 0x48F, 0x492, 0x496, 0x499, 0x49C, 0x49F, 0x4A2, 0x4A6, 0x4A9,
    0x4AC, 0x4AF, 0x4B2, 0x4B5, 0x4B7, 0x4BA, 0x4BD, 0x4C0, 0x4C3, 0x4C5, 0x4C8, 0x4CB, 0x4CD,
    0x4D0, 0x4D2, 0x4D5, 0x4D7, 0x4D9, 0x4DC, 0x4DE, 0x4E0, 0x4E3, 0x4E5, 0x4E7, 0x4E9, 0x4EB,
    0x4ED, 0x4EF, 0x4F1, 0x4F3, 0x4F5, 0x4F6, 0x4F8, 0x4FA, 0x4FB, 0x4FD, 0x4FF, 0x500, 0x502,
    0x503, 0x504, 0x506, 0x507, 0x508, 0x50A, 0x50B, 0x50C, 0x50D, 0x50E, 0x50F, 0x510, 0x511,
    0x511, 0x512, 0x513, 0x514, 0x514, 0x515, 0x516, 0x516, 0x517, 0x517, 0x517, 0x518, 0x518,
    0x518, 0x518, 0x518, 0x519, 0x519,
];

const DSP_COUNTER_MASKS: [u16; 32] = [
    0x0000, 0xFFE0, 0x3FF8, 0x1FE7, 0x7FE0, 0x1FF8, 0x0FE7, 0x3FE0, 0x0FF8, 0x07E7, 0x1FE0, 0x07F8,
    0x03E7, 0x0FE0, 0x03F8, 0x01E7, 0x07E0, 0x01F8, 0x00E7, 0x03E0, 0x00F8, 0x0067, 0x01E0, 0x0078,
    0x0027, 0x00E0, 0x0038, 0x0007, 0x0060, 0x0018, 0x0020, 0x0000,
];

const DSP_COUNTER_XORS: [u16; 32] = [
    0xFFFF, 0x0000, 0x3E08, 0x1D04, 0x0000, 0x1E08, 0x0D04, 0x0000, 0x0E08, 0x0504, 0x0000, 0x0608,
    0x0104, 0x0000, 0x0208, 0x0104, 0x0000, 0x0008, 0x0004, 0x0000, 0x0008, 0x0004, 0x0000, 0x0008,
    0x0004, 0x0000, 0x0008, 0x0004, 0x0000, 0x0008, 0x0000, 0x0000,
];

// 0x2f BRA: the 2 instead of 4 cycles are on purpose.
//           `branch_rel` will increment the cycle count
#[rustfmt::skip]
static CYCLES: [Cycles; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       2, 8, 4, 5, 3, 4, 3, 6,   2, 6, 5, 4, 5, 4, 6, 8,  // 0^
       2, 8, 4, 5, 4, 5, 5, 6,   5, 5, 6, 5, 2, 2, 4, 6,  // 1^
       2, 8, 4, 5, 3, 4, 3, 6,   2, 6, 5, 4, 5, 4, 5, 2,  // 2^
       2, 8, 4, 5, 4, 5, 5, 6,   5, 5, 6, 5, 2, 2, 3, 8,  // 3^
       2, 8, 4, 5, 3, 4, 3, 6,   2, 6, 4, 4, 5, 4, 6, 6,  // 4^
       2, 8, 4, 5, 4, 5, 5, 6,   5, 5, 4, 5, 2, 2, 4, 3,  // 5^
       2, 8, 4, 5, 3, 4, 3, 6,   2, 6, 4, 4, 5, 4, 5, 5,  // 6^
       2, 8, 4, 5, 4, 5, 5, 6,   5, 5, 5, 5, 2, 2, 3, 6,  // 7^
       2, 8, 4, 5, 3, 4, 3, 6,   2, 6, 5, 4, 5, 2, 4, 5,  // 8^
       2, 8, 4, 5, 4, 5, 5, 6,   5, 5, 5, 5, 2, 2,12, 5,  // 9^
       3, 8, 4, 5, 3, 4, 3, 6,   2, 6, 4, 4, 5, 2, 4, 4,  // a^
       2, 8, 4, 5, 4, 5, 5, 6,   5, 5, 5, 5, 2, 2, 3, 4,  // b^
       3, 8, 4, 5, 4, 5, 4, 7,   2, 5, 6, 4, 5, 2, 4, 9,  // c^
       2, 8, 4, 5, 5, 6, 6, 7,   4, 5, 5, 5, 2, 2, 6, 3,  // d^
       2, 8, 4, 5, 3, 4, 3, 6,   2, 4, 5, 3, 4, 3, 4, 2,  // e^
       2, 8, 4, 5, 4, 5, 5, 6,   3, 4, 5, 4, 2, 2, 4, 2,  // f^
];

const F0_RESET: u8 = 0x80;

/// Flags
pub mod flags {
    pub const CARRY: u8 = 0x01;
    pub const ZERO: u8 = 0x02;
    pub const INTERRUPT_ENABLE: u8 = 0x04;
    pub const HALF_CARRY: u8 = 0x08;
    pub const BREAK: u8 = 0x10;
    /// 0 means zero page is at 0x00xx,
    /// 1 means zero page is at 0x01xx
    pub const ZERO_PAGE: u8 = 0x20;
    pub const OVERFLOW: u8 = 0x40;
    pub const SIGN: u8 = 0x80;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum AdsrPeriod {
    Attack = 0,
    Decay = 1,
    Sustain = 2,
    Release = 3,
}

impl save_state::InSaveState for AdsrPeriod {
    fn serialize(&self, state: &mut SaveStateSerializer) {
        (*self as u8).serialize(state)
    }

    fn deserialize(&mut self, state: &mut SaveStateDeserializer) {
        let mut i: u8 = 0;
        i.deserialize(state);
        *self = match i {
            0 => Self::Attack,
            1 => Self::Decay,
            2 => Self::Sustain,
            3 => Self::Release,
            _ => panic!("unknown enum discriminant {}", i),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, InSaveState)]
pub struct StereoSample<T: save_state::InSaveState = i16> {
    pub l: T,
    pub r: T,
}

macro_rules! impl_int_stereo_sample {
    ($t:ty) => {
        impl StereoSample<$t> {
            pub const fn new2(v: $t) -> Self {
                Self::new(v, v)
            }

            pub const fn new(l: $t, r: $t) -> Self {
                Self { l, r }
            }

            pub fn wrapping_add(self, rhs: Self) -> Self {
                self.zip_with(rhs, <$t>::wrapping_add)
            }
        }
    };
}

impl_int_stereo_sample!(i16);
impl_int_stereo_sample!(i32);

impl StereoSample {
    pub fn to32(self) -> StereoSample<i32> {
        StereoSample::<i32>::new(self.l.into(), self.r.into())
    }
}

impl StereoSample<i32> {
    pub fn clip16(self) -> StereoSample<i16> {
        self.map(|c| ((c as u32) & 0xffff) as i16)
    }

    pub fn clamp16(self) -> StereoSample<i16> {
        self.map(|c| c.clamp(-0x8000, 0x7fff) as i16)
    }
}

impl<T: save_state::InSaveState> StereoSample<T> {
    pub fn map<U: save_state::InSaveState, F: FnMut(T) -> U>(self, mut f: F) -> StereoSample<U> {
        StereoSample {
            l: f(self.l),
            r: f(self.r),
        }
    }

    pub fn zip_with<
        T2: save_state::InSaveState,
        U: save_state::InSaveState,
        F: FnMut(T, T2) -> U,
    >(
        self,
        rhs: StereoSample<T2>,
        mut f: F,
    ) -> StereoSample<U> {
        StereoSample {
            l: f(self.l, rhs.l),
            r: f(self.r, rhs.r),
        }
    }
}

impl core::ops::Add<StereoSample> for StereoSample {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        self.zip_with(rhs, i16::saturating_add)
    }
}

impl core::ops::Add<StereoSample<i32>> for StereoSample<i32> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        self.zip_with(rhs, i32::saturating_add)
    }
}

fn load16<T: Into<usize> + Copy, const N: usize>(mem: &[u8; N], addr: T) -> u16 {
    u16::from_le_bytes([mem[addr.into() & (N - 1)], mem[(addr.into() + 1) & (N - 1)]])
}

fn exp_decrease(val: &mut i16) {
    *val = val.saturating_sub((val.saturating_sub(1) >> 8) + 1);
}

mod regs {
    pub const VOLL: u8 = 0;
    pub const PITCHL: u8 = 2;
    pub const PITCHH: u8 = 3;
    pub const SRCN: u8 = 4;
    pub const ADSR1: u8 = 5;
    pub const ADSR2: u8 = 6;
    pub const GAIN: u8 = 7;
    pub const ENVX: u8 = 8;
    pub const OUTX: u8 = 9;
    pub const FIR: u8 = 15;

    pub const MVOLL: u8 = 0x0c;
    pub const EFB: u8 = 0x0d;
    pub const EVOLL: u8 = 0x2c;
    pub const PMON: u8 = 0x2d;
    pub const NON: u8 = 0x3d;
    pub const KON: u8 = 0x4c;
    pub const EON: u8 = 0x4d;
    pub const KOFF: u8 = 0x5c;
    pub const DIR: u8 = 0x5d;
    pub const FLG: u8 = 0x6c;
    pub const ESA: u8 = 0x6d;
    pub const ENDX: u8 = 0x7c;
    pub const EDL: u8 = 0x7d;
}

#[derive(Debug, Clone, Copy, InSaveState)]
struct Voice {
    fade_in: u8,
    brr_base: u16,
    brr_index: u8,
    sample_offset: u8,
    gain: u16,
    prev_gain: u16,
    ipol_index: u16,
    envx_buf: u8,
    period: AdsrPeriod,
    decode_buffer: [i16; 12],
}

impl Voice {
    pub const fn new() -> Self {
        Self {
            fade_in: 0,
            brr_base: 0,
            brr_index: 0,
            sample_offset: 0,
            gain: 0,
            prev_gain: 0,
            ipol_index: 0,
            envx_buf: 0,
            period: AdsrPeriod::Release,
            decode_buffer: [0; 12],
        }
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct DspCounter(u16);

impl DspCounter {
    pub const fn new() -> Self {
        Self(0)
    }

    pub fn tick(&mut self) {
        self.0 ^= if self.0 & 0x7 == 0 { 5 } else { 0 };
        self.0 = (self.0 ^ if self.0 & 0x18 == 0 { 0x18 } else { 0 }).wrapping_sub(0x29);
    }

    pub const fn is_triggered(&self, rate: u8) -> bool {
        let ur = rate as usize;
        self.0 & DSP_COUNTER_MASKS[ur] == DSP_COUNTER_XORS[ur]
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct Dsp {
    mem: [u8; 0x80],
    step_counter: u8,
    counter: DspCounter,

    dir: u8,
    srcn: u8,
    dir_srcn: u16,
    pitch: u16,
    next_brr: u16,
    adsr: u8,
    pitch_modulation: u8,
    output: i16,
    noise_enabled: u8,
    noise: u16,
    looped_voice_bit: u8,
    echo_enabled: u8,
    echo_addr: u16,
    echo_index: u16,
    echo_length: u16,
    echo_ring_buf_addr: u8,
    echo_input: StereoSample<i32>,
    next_fade_in: u8,
    envx_buf: u8,
    outx_buf: u8,
    endx_buf: u8,
    flag_buf: u8,
    brr_head: u8,
    brr_data: u8,
    is_even: bool,
    fade_in_enable: u8,
    fade_out_enable: u8,

    voices: [Voice; 8],
    echo_history: [StereoSample; 8],
    echo_history_index: u8,
    main_sample: StereoSample,
    echo_sample: StereoSample,

    global_output: StereoSample,
}

impl Dsp {
    pub const fn new() -> Self {
        let mut mem = [0; 0x80];
        mem[regs::FLG as usize] = 0xe0;
        mem[regs::ENDX as usize] = 0xff;
        Self {
            mem,
            step_counter: 0,
            counter: DspCounter::new(),
            dir: 0,
            srcn: 0,
            dir_srcn: 0,
            pitch: 0,
            next_brr: 0,
            adsr: 0,
            pitch_modulation: 0,
            output: 0,
            noise_enabled: 0,
            noise: 0x4000,
            looped_voice_bit: 0,
            echo_enabled: 0,
            echo_addr: 0,
            echo_index: 0,
            echo_length: 0,
            echo_ring_buf_addr: 0,
            echo_input: StereoSample::<i32>::new2(0),
            next_fade_in: 0,
            envx_buf: 0,
            outx_buf: 0,
            endx_buf: 0,
            flag_buf: 0,
            brr_head: 0,
            brr_data: 0,
            is_even: true,
            fade_in_enable: 0,
            fade_out_enable: 0,
            voices: [Voice::new(); 8],
            echo_history: [StereoSample::<i16>::new2(0); 8],
            echo_history_index: 0,
            main_sample: StereoSample::<i16>::new2(0),
            echo_sample: StereoSample::<i16>::new2(0),

            global_output: StereoSample::<i16>::new2(0),
        }
    }

    pub fn write(&mut self, adr: u8, val: u8) {
        if adr < 0x80 {
            self.mem[usize::from(adr)] = match (adr, adr & 0xf) {
                (regs::KON, _) => {
                    self.next_fade_in = val;
                    val
                }
                (regs::ENDX, _) => {
                    self.endx_buf = 0;
                    0
                }
                (_, regs::ENVX) => {
                    self.envx_buf = val;
                    val
                }
                (_, regs::OUTX) => {
                    self.outx_buf = val;
                    val
                }
                _ => val,
            }
        }
    }

    pub const fn read(&self, adr: u8) -> u8 {
        self.mem[(adr & 0x7f) as usize]
    }

    pub fn run_step<const STEP: u8>(&mut self, voice: u8, ram: &[u8; MEMORY_SIZE]) {
        macro_rules! vx {
            ($id:ident) => {
                vx!($id | 0)
            };
            ($id:ident | $off:literal) => {
                self.mem[usize::from((voice << 4) | regs::$id | $off)]
            };
        }
        macro_rules! reg {
            ($id:ident) => {
                self.mem[usize::from(regs::$id)]
            };
        }
        macro_rules! ram {
            ($i:expr) => {
                ram[usize::from($i)]
            };
        }
        macro_rules! voice {
            () => {
                self.voices[usize::from(voice)]
            };
        }
        macro_rules! output {
            (left) => {
                output!(0 l)
            };
            (right) => {
                output!(1 r)
            };
            ($channel:literal $i:ident) => {{
                let sample =
                    ((i32::from(self.output) * i32::from(vx!(VOLL | $channel) as i8)) >> 7).clamp(-0x8000, 0x7fff) as i16;
                let amp = |s: &mut i16| *s = s.saturating_add(sample);
                amp(&mut self.main_sample.$i);
                if (self.echo_enabled >> voice) & 1 > 0 {
                    amp(&mut self.echo_sample.$i)
                }
            }};
        }
        match STEP {
            1 => {
                self.dir_srcn = (u16::from(self.srcn) << 2).wrapping_add(u16::from(self.dir) << 8);
                self.srcn = vx!(SRCN);
            }
            2 => {
                let addr = self
                    .dir_srcn
                    .wrapping_add(if voice!().fade_in > 0 { 0 } else { 2 });
                self.next_brr = load16(ram, addr);
                self.adsr = vx!(ADSR1);
                self.pitch = vx!(PITCHL).into();
            }
            3 => {
                self.run_step::<10>(voice, ram);
                self.run_step::<11>(voice, ram);
                self.run_step::<12>(voice, ram);
            }
            4 => {
                self.looped_voice_bit = 0;
                if voice!().ipol_index >= 0x4000 {
                    voice!().ipol_index &= 0x3fff;
                    let (shift, filter) = (self.brr_head >> 4, self.brr_head & 0b1100);
                    let new_brr_data = ram![voice!()
                        .brr_base
                        .wrapping_add(u16::from(voice!().brr_index))
                        .wrapping_add(2)];
                    let nibbles = [
                        self.brr_data >> 4,
                        self.brr_data & 0xf,
                        new_brr_data >> 4,
                        new_brr_data & 0xf,
                    ];
                    for nibble in nibbles {
                        let nibble = if nibble & 8 > 0 {
                            (nibble | 0xf0) as i8
                        } else {
                            nibble as i8
                        };
                        let sample = if shift <= 12 {
                            (i16::from(nibble) << shift) >> 1
                        } else if nibble < 0 {
                            -2048
                        } else {
                            0
                        };

                        let wsub = |n, s| usize::from(if n >= s { n - s } else { 12 + n - s });
                        let old = voice!().decode_buffer[wsub(voice!().sample_offset, 1)];
                        let older = voice!().decode_buffer[wsub(voice!().sample_offset, 2)];
                        let sample = (match filter {
                            0 => sample.into(),
                            0b0100 => {
                                i32::from(sample) + i32::from(old >> 1) - (i32::from(old) >> 5)
                            }
                            0b1000 => {
                                i32::from(sample) + i32::from(old) + ((-3 * i32::from(old)) >> 6)
                                    - i32::from(older >> 1)
                                    + i32::from(older >> 5)
                            }
                            0b1100 => {
                                i32::from(sample) + i32::from(old) + ((-13 * i32::from(old)) >> 7)
                                    - i32::from(older >> 1)
                                    + ((i32::from(older) * 3) >> 5)
                            }
                            _ => unreachable!(),
                        })
                        .clamp(-0x8000, 0x7fff) as i16;
                        let sample = ((sample as u16) << 1) as i16;
                        voice!().decode_buffer[usize::from(voice!().sample_offset)] = sample;
                        let so = &mut voice!().sample_offset;
                        *so = if *so > 10 { 0 } else { *so + 1 };
                    }
                    voice!().brr_index += 2;
                    if voice!().brr_index >= 8 {
                        voice!().brr_index = 0;
                        voice!().brr_base = voice!().brr_base.wrapping_add(9);
                        if self.brr_head & 1 > 0 {
                            voice!().brr_base = self.next_brr;
                            self.looped_voice_bit = 1 << voice;
                        }
                    }
                }
                voice!().ipol_index = voice!()
                    .ipol_index
                    .saturating_add(self.pitch)
                    .clamp(0, 0x7fff);
                output!(left);
            }
            5 => {
                output!(right);
                self.endx_buf = reg!(ENDX) | self.looped_voice_bit;
                if voice!().fade_in == 5 {
                    self.endx_buf &= !(1 << voice)
                }
            }
            6 => self.outx_buf = ((self.output as u16) >> 8) as u8,
            7 => {
                reg!(ENDX) = self.endx_buf;
                self.envx_buf = voice!().envx_buf;
            }
            8 => {
                vx!(OUTX) = self.outx_buf;
            }
            9 => {
                vx!(ENVX) = self.envx_buf;
            }
            10 => self.pitch = (self.pitch & 0xff) | (u16::from(vx!(PITCHH) & 0x3f) << 8),
            11 => {
                let base = u16::from(voice!().brr_base);
                let idx = u16::from(voice!().brr_index);
                self.brr_data = ram![base.wrapping_add(idx).wrapping_add(1)];
                self.brr_head = ram![base];
            }
            12 => {
                if voice != 0 && (self.pitch_modulation >> voice) & 1 > 0 {
                    self.pitch = self.pitch.wrapping_add(
                        ((i32::from(self.output >> 5) * i32::from(self.pitch)) >> 10) as i16 as u16,
                    );
                }
                if voice!().fade_in > 0 {
                    if voice!().fade_in == 5 {
                        voice!().brr_base = self.next_brr;
                        voice!().brr_index = 0;
                        voice!().sample_offset = 0;
                        self.brr_head = 0;
                    }
                    voice!().gain = 0;
                    voice!().prev_gain = 0;
                    voice!().ipol_index = 0;
                    voice!().fade_in -= 1;
                    if voice!().fade_in & 3 > 0 {
                        voice!().ipol_index = 0x4000
                    }
                    self.pitch = 0;
                }

                let gauss = (voice!().ipol_index >> 4) & 0xff;
                let off = voice!()
                    .sample_offset
                    .wrapping_add((voice!().ipol_index >> 12) as u8);
                let gv = |g, i| {
                    (i32::from(GAUSS_INTERPOLATION_POINTS[usize::from(g)])
                        * i32::from(voice!().decode_buffer[usize::from(i % 12)]))
                        >> 11
                };
                let out = if (self.noise_enabled >> voice) & 1 == 0 {
                    ((i32::from(
                        ((gv(0xff - gauss, off)
                            + gv(0x1ff - gauss, off + 1)
                            + gv(0x100 + gauss, off + 2)) as u32
                            & 0xffff) as i16,
                    ) + gv(gauss, off + 3))
                    .clamp(-0x8000, 0x7fff)) as i16
                } else {
                    (self.noise << 1) as i16
                };
                self.output = ((i32::from(out) * i32::from(voice!().gain)) >> 11) as i16;
                voice!().envx_buf = ((voice!().gain >> 4) & 0xff) as u8;
                if reg!(FLG) & 0x80 > 0 || self.brr_head & 3 == 1 {
                    voice!().period = AdsrPeriod::Release;
                    voice!().gain = 0;
                }

                if self.is_even && (self.fade_in_enable >> voice) & 1 > 0 {
                    voice!().fade_in = 5;
                    voice!().period = AdsrPeriod::Attack;
                } else if self.is_even && (self.fade_out_enable >> voice) & 1 > 0 {
                    voice!().period = AdsrPeriod::Release;
                }

                if voice!().fade_in == 0 {
                    if let AdsrPeriod::Release = voice!().period {
                        voice!().gain = voice!().gain.saturating_sub(8);
                    } else {
                        let mut gain = voice!().gain as i16;
                        let rate = if self.adsr & 0x80 > 0 {
                            // Attack/Decay/Sustain Mode
                            match voice!().period {
                                AdsrPeriod::Decay | AdsrPeriod::Sustain => {
                                    exp_decrease(&mut gain);
                                    if let AdsrPeriod::Decay = voice!().period {
                                        ((self.adsr >> 3) & 0xe) | 0x10
                                    } else {
                                        vx!(ADSR2) & 0x1f
                                    }
                                }
                                AdsrPeriod::Attack => {
                                    let rate = ((self.adsr & 0xf) << 1) | 1;
                                    gain = gain.saturating_add(if rate == 31 { 1024 } else { 32 });
                                    rate
                                }
                                _ => unreachable!(),
                            }
                        } else {
                            let new_gain = vx!(GAIN);
                            if new_gain & 0x80 > 0 {
                                // Custom Gain
                                match new_gain & 0x60 {
                                    0 => {
                                        // Linear Decrease
                                        gain = gain.saturating_sub(32)
                                    }
                                    0x20 => {
                                        // Exp Decrease
                                        exp_decrease(&mut gain);
                                    }
                                    0x40 => {
                                        // Linear Increase
                                        gain = gain.saturating_add(32)
                                    }
                                    0x60 => {
                                        // Bent Increase
                                        let d = if voice!().prev_gain >= 0x600 { 8 } else { 32 };
                                        gain = gain.saturating_add(d)
                                    }
                                    _ => unreachable!(),
                                }
                                new_gain & 0x1f
                            } else {
                                // Direct Gain
                                gain = (new_gain as i16) << 4;
                                31
                            }
                        };
                        if let AdsrPeriod::Decay = voice!().period {
                            let boundary = if self.adsr & 0x80 > 0 {
                                vx!(ADSR2)
                            } else {
                                vx!(GAIN)
                            };
                            if (gain >> 8) == i16::from(boundary >> 5) {
                                voice!().period = AdsrPeriod::Sustain
                            }
                        }
                        if gain > 0x7ff || gain < 0 {
                            if let AdsrPeriod::Attack = voice!().period {
                                voice!().period = AdsrPeriod::Decay
                            }
                        }
                        if self.counter.is_triggered(rate) {
                            voice!().gain = gain.clamp(0, 0x7ff) as u16
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn get_fir<const I: u8>(&self) -> StereoSample<i32> {
        let fir = i32::from(self.mem[usize::from(regs::FIR | (I << 4))] as i8);
        self.echo_history[usize::from(self.echo_history_index.wrapping_add(I + 1) & 7)]
            .map(|c| (i32::from(c) * fir) >> 6)
    }

    pub fn run_one_step(&mut self, ram: &mut [u8; MEMORY_SIZE]) {
        macro_rules! step {
            ($v:literal[$s:literal] $(, $v_:literal[$s_:literal])*) => {{
                self.run_step::<$s>($v & 7, ram);
                $(step!($v_[$s_]));*
            }};
        }
        macro_rules! reg {
            ($id:ident) => {
                self.mem[usize::from(regs::$id)]
            };
        }
        macro_rules! load_echo_history {
            (left) => { load_echo_history!(l 0) };
            (right) => { load_echo_history!(r 2) };
            ($i:ident $off:literal) => {
                let sample = load16(ram, self.echo_addr.wrapping_add($off)) as i16;
                self.echo_history[usize::from(self.echo_history_index)].$i = sample >> 1;
            };
        }
        macro_rules! calculate_echo {
            (left) => { calculate_echo!(l 0) };
            (right) => { calculate_echo!(r 16) };
            (part $i:ident $off:literal $first:ident $reg:ident) => {
                ((i32::from(self.$first.$i)
                  * i32::from(self.mem[usize::from(regs::$reg | $off)] as i8)
                 ) >> 7
                ) as i16
            };
            ($i:ident $off:literal) => {{
                calculate_echo!(part $i $off main_sample MVOLL)
                    .saturating_add(
                calculate_echo!(part $i $off echo_input EVOLL)
                )
            }};
        }
        macro_rules! echo_to_ram {
            (left) => { echo_to_ram!(l 0) };
            (right) => { echo_to_ram!(r 2) };
            ($i:ident $off:literal) => {{
                let sample = take(&mut self.echo_sample.$i);
                if self.flag_buf & 0x20 == 0 {
                    let adr = self.echo_addr.wrapping_add($off);
                    let [low, high] = sample.to_le_bytes();
                    ram[usize::from(adr)] = low;
                    ram[usize::from(adr.wrapping_add(1))] = high;
                }
            }};
        }
        match self.step_counter {
            0 => step!(0[5], 1[2]),
            1 => step!(0[6], 1[3]),
            2 => step!(0[7], 1[4], 3[1]),
            3 => step!(0[8], 1[5], 2[2]),
            4 => step!(0[9], 1[6], 2[3]),
            5 => step!(1[7], 2[4], 4[1]),
            6 => step!(1[8], 2[5], 3[2]),
            7 => step!(1[9], 2[6], 3[3]),
            8 => step!(2[7], 3[4], 5[1]),
            9 => step!(2[8], 3[5], 4[2]),
            10 => step!(2[9], 3[6], 4[3]),
            11 => step!(3[7], 4[4], 6[1]),
            12 => step!(3[8], 4[5], 5[2]),
            13 => step!(3[9], 4[6], 5[3]),
            14 => step!(4[7], 5[4], 7[1]),
            15 => step!(4[8], 5[5], 6[2]),
            16 => step!(4[9], 5[6], 6[3]),
            17 => step!(5[7], 6[4], 0[1]),
            18 => step!(5[8], 6[5], 7[2]),
            19 => step!(5[9], 6[6], 7[3]),
            20 => step!(6[7], 7[4], 1[1]),
            21 => step!(6[8], 7[5], 0[2]),
            22 => {
                step!(6[9], 7[6], 0[10]);
                self.echo_history_index = (self.echo_history_index + 1) & 7;
                self.echo_addr =
                    (self.echo_index << 4).wrapping_add(u16::from(self.echo_ring_buf_addr) << 8);
                load_echo_history!(left);
                self.echo_input = self.get_fir::<0>();
            }
            23 => {
                step!(7[7]);
                self.echo_input = self.echo_input + self.get_fir::<1>() + self.get_fir::<2>();
                load_echo_history!(right);
            }
            24 => {
                step!(7[8]);
                self.echo_input = self.echo_input
                    + self.get_fir::<3>()
                    + self.get_fir::<4>()
                    + self.get_fir::<5>();
            }
            25 => {
                step!(0[11], 7[9]);
                self.echo_input = ((self.echo_input + self.get_fir::<6>()).clip16().to32()
                    + self.get_fir::<7>().clip16().to32())
                .clamp16()
                .to32();
            }
            26 => {
                self.main_sample.l = calculate_echo!(left);

                let efb = i32::from(reg!(EFB) as i8);
                self.echo_sample = (self.echo_sample.to32()
                    + self.echo_input.map(|c| (c * efb) >> 7).clip16().to32())
                .clamp16();
            }
            27 => {
                self.pitch_modulation = reg!(PMON);

                self.main_sample.r = calculate_echo!(right);
                let out = take(&mut self.main_sample);

                self.global_output = if reg!(FLG) & 0x40 > 0 {
                    StereoSample::<i16>::new2(0)
                } else {
                    out
                };
            }
            28 => {
                self.dir = reg!(DIR);
                self.noise_enabled = reg!(NON);
                self.echo_enabled = reg!(EON);
                self.flag_buf = reg!(FLG);
            }
            29 => {
                self.is_even = !self.is_even;
                if self.is_even {
                    self.next_fade_in &= !self.fade_in_enable
                }

                self.echo_ring_buf_addr = reg!(ESA);
                if self.echo_index == 0 {
                    self.echo_length = u16::from(reg!(EDL) & 0xf) << 7;
                }
                self.echo_index += 1;
                if self.echo_index >= self.echo_length {
                    self.echo_index = 0
                }
                echo_to_ram!(left);
                self.flag_buf = reg!(FLG);
            }
            30 => {
                if self.is_even {
                    self.fade_in_enable = self.next_fade_in;
                    self.fade_out_enable = reg!(KOFF);
                }
                self.counter.tick();
                if self.counter.is_triggered(reg!(FLG) & 0x1f) {
                    self.noise = (((self.noise ^ (self.noise >> 1)) & 1) << 14) ^ (self.noise >> 1);
                }
                step!(0[12]);
                echo_to_ram!(right);
            }
            31 => step!(0[4], 2[1]),
            _ => unreachable!(),
        }
        self.step_counter += 1;
        self.step_counter &= 0x1f;
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct Spc700 {
    mem: [u8; MEMORY_SIZE],
    /// data, the main processor sends to us
    pub input: [u8; 4],
    /// data, we send to the main processor
    pub output: [u8; 4],
    dsp: Dsp,

    a: u8,
    x: u8,
    y: u8,
    sp: u8,
    status: u8,
    pc: u16,

    timer_max: [u8; 3],
    // internal timer ticks ALL in 64kHz
    timers: [u8; 3],
    timer_enable: u8,
    counters: [Cell<u8>; 3],
    dispatch_counter: u16,
    cycles_ahead: Cycles,
    halt: bool,
}

impl Default for Spc700 {
    fn default() -> Self {
        const fn generate_power_up_memory() -> [u8; MEMORY_SIZE] {
            let mut mem = [0; MEMORY_SIZE];
            mem[0xf0] = F0_RESET;
            mem
        }
        const POWER_UP_MEMORY: [u8; MEMORY_SIZE] = generate_power_up_memory();
        Self {
            mem: POWER_UP_MEMORY,
            input: [0; 4],
            output: [0; 4],
            dsp: Dsp::new(),
            a: 0,
            x: 0,
            y: 0,
            sp: 0xef,
            pc: 0xffc0,
            status: 2,

            timer_max: [0; 3],
            timers: [0; 3],
            timer_enable: 0,
            counters: [Cell::new(0), Cell::new(0), Cell::new(0)],
            dispatch_counter: 0,
            cycles_ahead: 2,
            halt: false,
        }
    }
}

impl Spc700 {
    pub fn reset(&mut self) {
        self.mem[0xf0] = F0_RESET;
        self.input = [0; 4];
        self.output = [0; 4];
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0;
        // actually self.read16(0xfffe), but this will
        // always result in 0xffc0, because mem[0xf0] = 0x80
        self.pc = 0xffc0;
        self.status = 0;
        self.halt = false;
        // TODO: reset dsp
    }

    pub fn is_rom_mapped(&self) -> bool {
        self.mem[0xf0] & 0x80 > 0
    }

    pub fn read16(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read(addr), self.read(addr.wrapping_add(1))])
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xf3 => self.dsp.read(self.mem[0xf2]),
            0xf4..=0xf7 => self.input[usize::from(addr - 0xf4)],
            0xfd..=0xff => self.counters[usize::from(addr - 0xfd)].take(),
            0xf0..=0xf1 | 0xfa..=0xfc => 0,
            0xffc0..=0xffff if self.is_rom_mapped() => ROM[(addr & 0x3f) as usize],
            addr => self.mem[addr as usize],
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xf0 => todo!("undocumented SPC register TEST(f0) written"),
            0xf1 => {
                if val & 0x10 > 0 {
                    self.input[0..2].fill(0)
                }
                if val & 0x20 > 0 {
                    self.input[2..4].fill(0)
                }
                let active = val & !self.timer_enable;
                self.timer_enable = val & 7;
                for i in 0..3 {
                    if active & (1 << i) > 0 {
                        self.counters[i].set(0);
                        self.timers[i] = 0;
                    }
                }
            }
            0xf3 => self.dsp.write(self.mem[0xf2], val),
            0xf4..=0xf7 => self.output[(addr - 0xf4) as usize] = val,
            0xfa..=0xfc => self.timer_max[usize::from(addr & 3) ^ 2] = val,
            addr => self.mem[addr as usize] = val,
        }
    }

    pub fn get_small(&self, addr: u8) -> u16 {
        u16::from(addr) | (((self.status & flags::ZERO_PAGE) as u16) << 3)
    }

    pub fn read_small(&self, addr: u8) -> u8 {
        self.read(self.get_small(addr))
    }

    pub fn read16_small(&self, addr: u8) -> u16 {
        u16::from_le_bytes([self.read_small(addr), self.read_small(addr.wrapping_add(1))])
    }

    pub fn write16(&mut self, addr: u16, val: u16) {
        let [a, b] = val.to_le_bytes();
        self.write(addr, a);
        self.write(addr.wrapping_add(1), b);
    }

    pub fn write_small(&mut self, addr: u8, val: u8) {
        self.write(self.get_small(addr), val)
    }

    pub fn write16_small(&mut self, addr: u8, val: u16) {
        let [a, b] = val.to_le_bytes();
        self.write_small(addr, a);
        self.write_small(addr.wrapping_add(1), b)
    }

    pub fn push(&mut self, val: u8) {
        self.write(u16::from(self.sp) | 0x100, val);
        self.sp = self.sp.wrapping_sub(1);
    }

    pub fn push16(&mut self, val: u16) {
        let [a, b] = val.to_be_bytes();
        self.push(a);
        self.push(b)
    }

    pub fn pull(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.read(u16::from(self.sp) | 0x100)
    }

    pub fn pull16(&mut self) -> u16 {
        u16::from_le_bytes([self.pull(), self.pull()])
    }

    pub fn load(&mut self) -> u8 {
        let val = self.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    pub fn load16(&mut self) -> u16 {
        let val = self.read16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        val
    }

    pub fn ya(&self) -> u16 {
        u16::from_le_bytes([self.a, self.y])
    }

    pub fn set_ya(&mut self, val: u16) {
        let [a, y] = val.to_le_bytes();
        self.a = a;
        self.y = y;
    }

    pub fn set_status(&mut self, cond: bool, flag: u8) {
        if cond {
            self.status |= flag
        } else {
            self.status &= !flag
        }
    }

    pub fn dispatch_instruction(&mut self) -> Cycles {
        let op = self.load();
        let mut cycles = CYCLES[op as usize];
        match op {
            0x00 => (), // NOP
            0x01 | 0x11 | 0x21 | 0x31 | 0x41 | 0x51 | 0x61 | 0x71 | 0x81 | 0x91 | 0xa1 | 0xb1
            | 0xc1 | 0xd1 | 0xe1 | 0xf1 => {
                // TCALL n
                self.push16(self.pc);
                self.pc = self.read16(0xffde ^ (u16::from(op & 0xf) << 1));
            }
            0x02 | 0x22 | 0x42 | 0x62 | 0x82 | 0xa2 | 0xc2 | 0xe2 => {
                // SET1 - (imm) |= 1 << ?
                let addr = self.load();
                let addr = self.get_small(addr);
                self.write(addr, self.read(addr) | 1 << (op >> 5))
            }
            0x12 | 0x32 | 0x52 | 0x72 | 0x92 | 0xb2 | 0xd2 | 0xf2 => {
                // CLR1 - (imm) &= ~(1 << ?)
                let addr = self.load();
                let addr = self.get_small(addr);
                self.write(addr, self.read(addr) & !(1 << (op >> 5)))
            }
            0x03 | 0x23 | 0x43 | 0x63 | 0x83 | 0xa3 | 0xc3 | 0xe3 | 0x13 | 0x33 | 0x53 | 0x73
            | 0x93 | 0xb3 | 0xd3 | 0xf3 => {
                // Branch if bit set/cleared
                let addr = self.load();
                let val = self.read_small(addr);
                let rel = self.load();
                self.branch_rel(rel, ((val >> (op >> 5)) ^ (op >> 4)) & 1 == 1, &mut cycles);
            }
            0x04 => {
                // OR - A |= (imm)
                let addr = self.load();
                self.a |= self.read_small(addr);
                self.update_nz8(self.a);
            }
            0x05 => {
                // OR - A |= (imm[16-bit])
                let addr = self.load16();
                self.a |= self.read(addr);
                self.update_nz8(self.a);
            }
            0x06 => {
                // OR - A |= (X)
                self.a |= self.read_small(self.x);
                self.update_nz8(self.a);
            }
            0x07 => {
                // OR - A |= ((imm + X)[16-bit])
                let addr = self.load().wrapping_add(self.x);
                self.a |= self.read(self.read16_small(addr));
                self.update_nz8(self.a);
            }
            0x08 => {
                // OR - A |= imm
                self.a |= self.load();
                self.update_nz8(self.a)
            }
            0x09 => {
                // OR - (imm) |= (imm)
                let (src, dst) = (self.load(), self.load());
                let dst = self.get_small(dst);
                let val = self.read_small(src) | self.read(dst);
                self.write(dst, val);
                self.update_nz8(val);
            }
            0x0a => {
                // OR1 - OR CARRY on (imm2) >> imm1
                let addr = self.load16();
                let val = self.read(addr & 0x1fff);
                self.status |= (val >> (addr >> 13)) & flags::CARRY
            }
            0x0b => {
                // ASL - (imm) <<= 1
                let addr = self.load();
                let addr = self.get_small(addr);
                let mut val = self.read(addr);
                self.set_status(val >= 0x80, flags::CARRY);
                val <<= 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x0c => {
                // ASL - (a) <<= 1
                let addr = self.load16();
                let mut val = self.read(addr);
                self.set_status(val >= 0x80, flags::CARRY);
                val <<= 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x0d => {
                // PUSH - status
                self.push(self.status)
            }
            0x0e => {
                // TSET1 - (imm[16-bit]) |= A
                let addr = self.load16();
                let val = self.read(addr);
                self.update_nz8(self.a.wrapping_add(!val).wrapping_add(1));
                self.write(addr, val | self.a)
            }
            0x0f => {
                // BRK - Push PC and Status and go to interrupt vector 0xffde
                let new_pc = self.read16(0xffde);
                self.push16(self.pc);
                self.pc = new_pc;
                self.status = (self.status | flags::BREAK) & !flags::INTERRUPT_ENABLE
            }
            0x10 => {
                // BPL/JNS - Branch if SIGN not set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::SIGN == 0, &mut cycles)
            }
            0x14 => {
                // OR - A |= (imm + X)
                let addr = self.load().wrapping_add(self.x);
                self.a |= self.read_small(addr);
                self.update_nz8(self.a);
            }
            0x15 => {
                // OR - A |= (imm[16-bit] + X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.a |= self.read(addr);
                self.update_nz8(self.a);
            }
            0x16 => {
                // OR - A |= (imm[16-bit] + Y)
                let addr = self.load16().wrapping_add(self.y.into());
                self.a |= self.read(addr);
                self.update_nz8(self.a);
            }
            0x17 => {
                // OR - A |= ((imm)[16-bit] + Y)
                let addr = self.load();
                self.a |= self.read(self.read16_small(addr).wrapping_add(self.y.into()));
                self.update_nz8(self.a);
            }
            0x18 => {
                // OR - (imm) |= imm
                let (src, dst) = (self.load(), self.load());
                let dst = self.get_small(dst);
                let val = src | self.read(dst);
                self.write(dst, val);
                self.update_nz8(val);
            }
            0x19 => {
                // OR - (X) |= (Y)
                let x = self.get_small(self.x);
                let res = self.read(x) | self.read_small(self.y);
                self.write(x, res);
                self.update_nz8(res)
            }
            0x1a => {
                // DECW - (imm)[16-bit]--
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read16(addr).wrapping_sub(1);
                self.write16(addr, val);
                self.update_nz16(val)
            }
            0x1b => {
                // ASL - (imm + X) <<= 1
                let addr = self.load().wrapping_add(self.x);
                let addr = self.get_small(addr);
                let val = self.read(addr);
                self.set_status(val >= 0x80, flags::CARRY);
                let val = val << 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x1c => {
                // ASL - A <<= 1
                self.set_status(self.a >= 0x80, flags::CARRY);
                self.a <<= 1;
                self.update_nz8(self.a)
            }
            0x1d => {
                // DEC - X
                self.x = self.x.wrapping_sub(1);
                self.update_nz8(self.x);
            }
            0x1e => {
                // CMP - X - (imm)
                let addr = self.load16();
                let val = self.read(addr);
                self.compare(self.x, val)
            }
            0x1f => {
                // JMP - PC := (X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.pc = self.read16(addr);
            }
            0x20 => {
                // CLRP - Clear ZERO_PAGE
                self.status &= !flags::ZERO_PAGE
            }
            0x24 => {
                // AND - A &= (imm)
                let addr = self.load();
                self.a &= self.read_small(addr);
                self.update_nz8(self.a)
            }
            0x25 => {
                // AND - A &= (imm[16-bit])
                let addr = self.load16();
                self.a &= self.read(addr);
                self.update_nz8(self.a)
            }
            0x26 => {
                // AND - A &= (X)
                self.a &= self.read_small(self.x);
                self.update_nz8(self.a)
            }
            0x27 => {
                // AND - A &= ((imm + X)[16-bit])
                let addr = self.load().wrapping_add(self.x);
                let addr = self.read16_small(addr);
                self.a &= self.read(addr);
                self.update_nz8(self.a)
            }
            0x28 => {
                // AND - A &= imm
                self.a &= self.load();
                self.update_nz8(self.a)
            }
            0x29 => {
                // AND - (imm) &= (imm)
                let src = self.load();
                let dst = self.load();
                let [src, dst] = [src, dst].map(|v| self.get_small(v));
                let val = self.read(src) & self.read(dst);
                self.write(dst, val);
                self.update_nz8(val)
            }
            0x2a => {
                // OR1 - NOR CARRY on (imm2) >> imm1
                let addr = self.load16();
                let val = !self.read(addr & 0x1fff);
                self.status |= (val >> (addr >> 13)) & flags::CARRY
            }
            0x2b => {
                // ROL - (imm) <<= 1
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr);
                let new_val = (val << 1) | (self.status & flags::CARRY);
                self.set_status(val >= 0x80, flags::CARRY);
                self.write(addr, new_val);
                self.update_nz8(new_val);
            }
            0x2c => {
                // ROL - (imm[16-bit]) <<= 1
                let addr = self.load16();
                let val = self.read(addr);
                let new_val = (val << 1) | (self.status & flags::CARRY);
                self.set_status(val >= 0x80, flags::CARRY);
                self.write(addr, new_val);
                self.update_nz8(new_val);
            }
            0x2d => {
                // PUSH - A
                self.push(self.a)
            }
            0x2e => {
                // CBNE - Branch if A != (imm)
                let addr = self.load();
                let rel = self.load();
                self.branch_rel(rel, self.read_small(addr) != self.a, &mut cycles)
            }
            0x2f => {
                // BRA - Branch always
                let rel = self.load();
                self.branch_rel(rel, true, &mut cycles)
            }
            0x30 => {
                // BMI - Branch if SIGN is set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::SIGN > 0, &mut cycles)
            }
            0x34 => {
                // AND - A &= (imm+X)
                let addr = self.load().wrapping_add(self.x);
                self.a &= self.read_small(addr);
                self.update_nz8(self.a)
            }
            0x35 => {
                // AND - A &= (imm[16-bit] + X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.a &= self.read(addr);
                self.update_nz8(self.a);
            }
            0x36 => {
                // AND - A &= (imm[16-bit] + Y)
                let addr = self.load16().wrapping_add(self.y.into());
                self.a &= self.read(addr);
                self.update_nz8(self.a);
            }
            0x37 => {
                // AND - A &= ((imm)[16-bit] + Y)
                let addr = self.load();
                let addr = self.read16_small(addr);
                self.a &= self.read(addr.wrapping_add(self.y.into()));
                self.update_nz8(self.a);
            }
            0x38 => {
                // AND - (imm) &= imm
                let imm = self.load();
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr) & imm;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x39 => {
                // AND - (X) &= (Y)
                let addr = self.get_small(self.x);
                let val = self.read(addr) & self.read_small(self.y);
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x3a => {
                // INCW - (imm)[16-bit]++
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read16(addr).wrapping_add(1);
                self.write16(addr, val);
                self.update_nz16(val)
            }
            0x3b => {
                // ROL - (imm + X) <<= 1
                let addr = self.load().wrapping_add(self.x);
                let addr = self.get_small(addr);
                let val = self.read(addr);
                let new_val = (val << 1) | (self.status & flags::CARRY);
                self.set_status(val >= 0x80, flags::CARRY);
                self.write(addr, new_val);
                self.update_nz8(new_val);
            }
            0x3c => {
                // ROL - A <<= 1
                let c = self.a & 0x80;
                self.a = (self.a << 1) | (self.status & flags::CARRY);
                self.set_status(c > 0, flags::CARRY);
                self.update_nz8(self.a);
            }
            0x3d => {
                // INC - X
                self.x = self.x.wrapping_add(1);
                self.update_nz8(self.x);
            }
            0x3e => {
                // CMP - X - (imm)
                let addr = self.load();
                let val = self.read_small(addr);
                self.compare(self.x, val)
            }
            0x3f => {
                // CALL - Call a subroutine
                let addr = self.load16();
                self.push16(self.pc);
                self.pc = addr
            }
            0x40 => {
                // SETP - Set ZERO_PAGE
                self.status |= flags::ZERO_PAGE
            }
            0x44 => {
                // EOR - A := A ^ (imm)
                let addr = self.load();
                self.a ^= self.read_small(addr);
                self.update_nz8(self.a)
            }
            0x45 => {
                // EOR - A := a ^ (imm[16-bit])
                let addr = self.load16();
                self.a ^= self.read(addr);
                self.update_nz8(self.a)
            }
            0x46 => {
                // EOR - A ^= (X)
                let addr = self.load();
                self.a ^= self.read_small(addr);
                self.update_nz8(self.a)
            }
            0x47 => {
                // EOR - A ^= ((imm + X)[16-bit])
                let addr = self.load().wrapping_add(self.x);
                let addr = self.read16_small(addr);
                self.a ^= self.read(addr);
                self.update_nz8(self.a)
            }
            0x48 => {
                // EOR - A := A ^ imm
                self.a ^= self.load();
                self.update_nz8(self.a)
            }
            0x49 => {
                // EOR - (imm) ^= (imm)
                let (src, dst) = (self.load(), self.load());
                let dst = self.get_small(dst);
                let val = self.read_small(src) ^ self.read(dst);
                self.write(dst, val);
                self.update_nz8(val)
            }
            0x4a => {
                // AND1 - AND CARRY on (imm2) >> imm1
                let addr = self.load16();
                let val = self.read(addr & 0x1fff);
                self.status &= (val >> (addr >> 13)) & flags::CARRY
            }
            0x4b => {
                // LSR - (imm) >>= 1
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr);
                self.set_status(val & 1 > 0, flags::CARRY);
                let val = val >> 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x4c => {
                // LSR - (imm[16-bit]) >>= 1
                let addr = self.load16();
                let val = self.read(addr);
                self.set_status(val & 1 > 0, flags::CARRY);
                let val = val >> 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x4d => {
                // PUSH - X
                self.push(self.x)
            }
            0x4e => {
                // TCLR1
                let addr = self.load16();
                let val = self.read(addr);
                self.update_nz8(self.a.wrapping_add(!val).wrapping_add(1));
                self.write(addr, val & !self.a)
            }
            0x4f => {
                // PCALL
                let addr = self.load();
                self.push16(self.pc);
                self.pc = u16::from_le_bytes([addr, 0xff])
            }
            0x50 => {
                // BVC - Branch if V=0
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::OVERFLOW == 0, &mut cycles)
            }
            0x54 => {
                // EOR - A := A ^ (imm+X)
                let addr = self.load().wrapping_add(self.x);
                self.a ^= self.read_small(addr);
                self.update_nz8(self.a)
            }
            0x55 => {
                // EOR - A := A ^ (imm[16-bit]+X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.a ^= self.read(addr);
                self.update_nz8(self.a)
            }
            0x56 => {
                // EOR - A := A ^ (imm[16-bit]+Y)
                let addr = self.load16().wrapping_add(self.y.into());
                self.a ^= self.read(addr);
                self.update_nz8(self.a)
            }
            0x57 => {
                // EOR - A := A ^ ((imm)[16-bit]+Y)
                let addr = self.load();
                let addr = self.read16_small(addr).wrapping_add(self.y.into());
                self.a ^= self.read(addr);
                self.update_nz8(self.a)
            }
            0x58 => {
                // EOR - (imm) ^= imm
                let val = self.load();
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr) ^ val;
                self.write(addr, val);
                self.update_nz8(val);
            }
            0x59 => {
                // EOR - (X) ^= (Y)
                let addr = self.get_small(self.x);
                let res = self.read(addr) ^ self.read_small(self.y);
                self.write(addr, res);
                self.update_nz8(res)
            }
            0x5a => {
                // CMPW - YA - (imm)[16-bit]
                let val = self.load();
                let (result, ov1) = self.ya().overflowing_add(!self.read16_small(val));
                let (result, ov2) = result.overflowing_add(1);
                self.set_status(ov1 || ov2, flags::CARRY);
                self.update_nz16(result);
            }
            0x5b => {
                // LSR - (imm+X) >>= 1
                let addr = self.load().wrapping_add(self.x);
                let addr = self.get_small(addr);
                let val = self.read(addr);
                self.set_status(val & 1 > 0, flags::CARRY);
                let val = val >> 1;
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x5c => {
                // LSR - A >>= 1
                self.set_status(self.a & 1 > 0, flags::CARRY);
                self.a >>= 1;
                self.update_nz8(self.a)
            }
            0x5d => {
                // MOV - X := A
                self.x = self.a;
                self.update_nz8(self.x)
            }
            0x5e => {
                // CMP - Y - (imm[16-bit])
                let addr = self.load16();
                let val = self.read(addr);
                self.compare(self.y, val)
            }
            0x5f => {
                // JMP - PC := imm[16-bit]
                self.pc = self.load16();
            }
            0x60 => {
                // CLRC - Clear CARRY
                self.status &= !flags::CARRY
            }
            0x64 => {
                // CMP - A - (imm)
                let addr = self.load();
                let val = self.read_small(addr);
                self.compare(self.a, val)
            }
            0x65 => {
                // CMP - A - (imm[16-bit])
                let addr = self.load16();
                let val = self.read(addr);
                self.compare(self.a, val)
            }
            0x66 => {
                // CMP - A - (X)
                self.compare(self.a, self.read_small(self.x))
            }
            0x67 => {
                // CMP - A - ((imm + X)[16-bit])
                let addr = self.load().wrapping_add(self.x);
                let val = self.read(self.read16_small(addr));
                self.compare(self.a, val)
            }
            0x68 => {
                // CMP - A - imm
                let val = self.load();
                self.compare(self.a, val)
            }
            0x69 => {
                // CMP - (dp) - (dp)
                let val1 = self.load();
                let val1 = self.read_small(val1);
                let val2 = self.load();
                let val2 = self.read_small(val2);
                self.compare(val2, val1);
            }
            0x6a => {
                // AND1 - AND CARRY on !(imm2) >> imm1
                let addr = self.load16();
                let val = !self.read(addr & 0x1fff);
                self.status &= (val >> (addr >> 13)) & flags::CARRY
            }
            0x6b => {
                // ROR - (imm) >>= 1
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr);
                let new_val = (val >> 1) | ((self.status & flags::CARRY) << 7);
                self.status = (self.status & 0xfe) | (val & flags::CARRY);
                self.write(addr, new_val);
                self.update_nz8(new_val);
            }
            0x6c => {
                // ROR - (imm[16-bit]) >>= 1
                let addr = self.load16();
                let val = self.read(addr);
                let new_val = (val >> 1) | ((self.status & flags::CARRY) << 7);
                self.status = (self.status & 0xfe) | (val & flags::CARRY);
                self.write(addr, new_val);
                self.update_nz8(new_val);
            }
            0x6d => {
                // PUSH - Y
                self.push(self.y)
            }
            0x6e => {
                // DBNZ - (imm)--; JNZ
                let addr = self.load();
                let rel = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_sub(1);
                self.write(addr, val);
                self.branch_rel(rel, val > 0, &mut cycles)
            }
            0x6f => {
                // RET - Return from subroutine
                self.pc = self.pull16()
            }
            0x70 => {
                // BVS - Branch if V=1
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::OVERFLOW > 0, &mut cycles)
            }
            0x74 => {
                // CMP - A - (imm+X)
                let addr = self.load().wrapping_add(self.x);
                let val = self.read_small(addr);
                self.compare(self.a, val)
            }
            0x75 => {
                // CMP - A - (imm[16-bit]+X)
                let addr = self.load16().wrapping_add(self.x.into());
                let val = self.read(addr);
                self.compare(self.a, val)
            }
            0x76 => {
                // CMP - A - (imm[16-bit]+Y)
                let addr = self.load16().wrapping_add(self.y.into());
                let val = self.read(addr);
                self.compare(self.a, val)
            }
            0x77 => {
                // CMP - A - ((imm)[16-bit] + Y)
                let addr = self.load();
                let addr = self.read16_small(addr).wrapping_add(self.y.into());
                let val = self.read(addr);
                self.compare(self.a, val)
            }
            0x78 => {
                // CMP - (imm) - imm
                let (b, a) = (self.load(), self.load());
                let a = self.read_small(a);
                self.compare(a, b)
            }
            0x79 => {
                // CMP - (X) - (Y)
                let (x, y) = (self.read_small(self.x), self.read_small(self.y));
                self.compare(x, y)
            }
            0x7a => {
                // ADDW - YA += (imm)[16-bit]
                let addr = self.load();
                let val = self.read16_small(addr);
                let val = self.add16(self.ya(), val);
                self.set_ya(val);
            }
            0x7b => {
                // ROR - (imm + X) >>= 1
                let addr = self.load().wrapping_add(self.x);
                let addr = self.get_small(addr);
                let val = self.read(addr);
                let new_val = (val >> 1) | ((self.status & flags::CARRY) << 7);
                self.status = (self.status & 0xfe) | (val & flags::CARRY);
                self.write(addr, new_val);
                self.update_nz8(new_val);
            }
            0x7c => {
                // ROR - A >>= 1
                let new_a = (self.a >> 1) | ((self.status & flags::CARRY) << 7);
                self.status = (self.status & 0xfe) | (self.a & flags::CARRY);
                self.a = new_a;
                self.update_nz8(new_a);
            }
            0x7d => {
                // MOV - A := X
                self.a = self.x;
                self.update_nz8(self.a)
            }
            0x7e => {
                // CMP - Y - (imm)
                let addr = self.load();
                self.compare(self.y, self.read_small(addr))
            }
            0x7f => {
                // RETI - Pop Status, Pop PC
                self.status = self.pull();
                self.pc = self.pull16();
            }
            0x80 => {
                // SETC - Set CARRY
                self.status |= flags::CARRY
            }
            0x84 => {
                // ADC - A += (imm) + CARRY
                let addr = self.load();
                let val = self.read_small(addr);
                self.a = self.adc(self.a, val)
            }
            0x85 => {
                // ADC - A += (imm[16-bit]) + CARRY
                let addr = self.load16();
                let val = self.read(addr);
                self.a = self.adc(self.a, val)
            }
            0x86 => {
                // ADC - A += (X) + CARRY
                self.a = self.adc(self.a, self.read_small(self.x))
            }
            0x87 => {
                // ADC - A += ((imm+X)[16-bit]) + CARRY
                let addr = self.load().wrapping_add(self.x);
                self.a = self.adc(self.a, self.read(self.read16_small(addr)))
            }
            0x88 => {
                // ADC - A += imm + CARRY
                let val = self.load();
                self.a = self.adc(self.a, val)
            }
            0x89 => {
                // ADC - (imm) += (imm)
                let addr1 = self.load();
                let addr1 = self.get_small(addr1);
                let addr2 = self.load();
                let addr2 = self.get_small(addr2);
                let result = self.adc(self.read(addr2), self.read(addr1));
                self.write(addr2, result);
            }
            0x8a => {
                // EOR1 - XOR CARRY on (imm2) >> imm1
                let addr = self.load16();
                let val = self.read(addr & 0x1fff);
                self.status ^= (val >> (addr >> 13)) & flags::CARRY
            }
            0x8b => {
                // DEC - Decrement (imm)
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_sub(1);
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x8c => {
                // DEC - (imm[16-bit])--
                let addr = self.load16();
                let val = self.read(addr).wrapping_sub(1);
                self.write(addr, val);
                self.update_nz8(val)
            }
            0x8d => {
                // MOV - Y := IMM
                self.y = self.load();
                self.update_nz8(self.y);
            }
            0x8e => {
                // POP - status
                self.status = self.pull()
            }
            0x8f => {
                // MOV - (dp) := IMM
                let (val, addr) = (self.load(), self.load());
                self.write_small(addr, val);
            }
            0x90 => {
                // BCC - Branch if CARRY not set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::CARRY == 0, &mut cycles)
            }
            0x94 => {
                // ADC - A += (imm + X) + CARRY
                let addr = self.load().wrapping_add(self.x);
                self.a = self.adc(self.a, self.read_small(addr));
            }
            0x95 => {
                // ADC - A -= (imm16 + X) + CARRY
                let addr = self.load16().wrapping_add(self.x.into());
                self.a = self.adc(self.a, self.read(addr));
            }
            0x96 => {
                // ADC - A -= (imm16 + Y) + CARRY
                let addr = self.load16().wrapping_add(self.y.into());
                self.a = self.adc(self.a, self.read(addr));
            }
            0x97 => {
                // ADC - A += ((imm)[16-bit] + Y) + CARRY
                let addr = self.load();
                let addr = self.read16_small(addr).wrapping_add(self.y.into());
                self.a = self.adc(self.a, self.read(addr))
            }
            0x98 => {
                // ADC - (imm) += imm + CARRY
                let val = self.load();
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.adc(self.read(addr), val);
                self.write(addr, val)
            }
            0x99 => {
                // ADC - (X) += (Y) + CARRY
                let addr = self.get_small(self.x);
                let val = self.adc(self.read(addr), self.read_small(self.y));
                self.write(addr, val)
            }
            0x9a => {
                // SUBW - YA -= (imm)[16-bit]
                let addr = self.load();
                let val = self.read16_small(addr);
                self.status |= flags::CARRY;
                let val = self.adc16(self.ya(), !val);
                self.set_ya(val);
            }
            0x9b => {
                // DEC - (imm+X)[16-bit]--
                let addr = self.load().wrapping_add(self.x);
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_sub(1);
                self.write(addr, val);
                self.update_nz8(val);
            }
            0x9c => {
                // DEC - A
                self.a = self.a.wrapping_sub(1);
                self.update_nz8(self.a);
            }
            0x9d => {
                // MOV - X := SP
                self.x = self.sp;
                self.update_nz8(self.x);
            }
            0x9e => {
                // DIV - Y, A := YA % X, YA / X
                // TODO: no exact reproduction of behaviour (see bsnes impl)
                let (rdiv, rmod) = if self.x == 0 {
                    (0xffff, self.a)
                } else {
                    let ya = self.ya();
                    let x = u16::from(self.x);
                    (ya / x, (ya % x) as u8)
                };
                self.set_status(rdiv > 0xff, flags::OVERFLOW);
                // TODO: understand why this works and what exactly HALF_CARRY does
                // This will probably work, because bsnes does this
                self.set_status((self.x & 15) <= (self.y & 15), flags::HALF_CARRY);
                self.a = (rdiv & 0xff) as u8;
                self.y = rmod;
                self.update_nz8(self.a);
            }
            0x9f => {
                // XCN - A := (A >> 4) | (A << 4)
                self.a = (self.a >> 4) | (self.a << 4);
                self.update_nz8(self.a)
            }
            0xa0 => {
                // EI - Set INTERRUPT_ENABLE
                self.status |= flags::INTERRUPT_ENABLE
            }
            0xa4 => {
                // SBC - A -= (imm) + CARRY
                let addr = self.load();
                self.a = self.adc(self.a, !self.read_small(addr));
            }
            0xa5 => {
                // SBC - A -= (imm[16-bit]) + CARRY
                let addr = self.load16();
                self.a = self.adc(self.a, !self.read(addr));
            }
            0xa6 => {
                // ADC - A -= (X) + CARRY
                self.a = self.adc(self.a, !self.read_small(self.x))
            }
            0xa7 => {
                // SBC - A -= ((imm + X)[16-bit]) + CARRY
                let addr = self.load().wrapping_add(self.x);
                let val = self.read(self.read16_small(addr));
                self.a = self.adc(self.a, !val);
            }
            0xa8 => {
                // SBC - A -= imm + CARRY
                let val = self.load();
                self.a = self.adc(self.a, !val);
            }
            0xa9 => {
                // SBC - (imm) -= (imm) + CARRY
                let (src, dst) = (self.load(), self.load());
                let dst = self.get_small(dst);
                let res = self.adc(self.read(dst), self.read_small(src));
                self.write(dst, res);
                self.update_nz8(res)
            }
            0xaa => {
                // MOV1 - Set CARRY on (imm2) >> imm1
                let addr = self.load16();
                let val = self.read(addr & 0x1fff);
                self.status = (self.status & !flags::CARRY) | ((val >> (addr >> 13)) & flags::CARRY)
            }
            0xab => {
                // INC - Increment (imm)
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_add(1);
                self.write(addr, val);
                self.update_nz8(val)
            }
            0xac => {
                // INC - (imm[16-bit])++
                let addr = self.load16();
                let val = self.read(addr).wrapping_add(1);
                self.write(addr, val);
                self.update_nz8(val)
            }
            0xad => {
                // CMP - Y - IMM
                let val = self.load();
                self.compare(self.y, val)
            }
            0xae => {
                // POP - A
                self.a = self.pull()
            }
            0xaf => {
                // MOV - (X) := A; X++
                self.write_small(self.x, self.a);
                self.x = self.x.wrapping_add(1);
            }
            0xb0 => {
                // BCS - Jump if CARRY set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::CARRY > 0, &mut cycles)
            }
            0xb4 => {
                // SBC - A -= (imm + X) + CARRY
                let addr = self.load().wrapping_add(self.x);
                self.a = self.adc(self.a, !self.read_small(addr));
            }
            0xb5 => {
                // SBC - A -= (imm16 + X) + CARRY
                let addr = self.load16().wrapping_add(self.x.into());
                self.a = self.adc(self.a, !self.read(addr));
            }
            0xb6 => {
                // SBC - A -= (imm16 + Y) + CARRY
                let addr = self.load16().wrapping_add(self.y.into());
                self.a = self.adc(self.a, !self.read(addr));
            }
            0xb7 => {
                // SBC - A -= ((imm)[16-bit] + Y) + CARRY
                let addr = self.load();
                let addr = self.read16_small(addr).wrapping_add(self.y.into());
                self.a = self.adc(self.a, !self.read(addr));
            }
            0xb8 => {
                // SBC - (imm) -= imm + CARRY
                let (val, dst) = (self.load(), self.load());
                let dst = self.get_small(dst);
                let res = self.adc(self.read(dst), !val);
                self.write(dst, res);
            }
            0xb9 => {
                // SBC - (X) -= (Y) + CARRY
                let addr = self.get_small(self.x);
                let val = self.adc(self.read(addr), !self.read_small(self.y));
                self.write(addr, val);
            }
            0xba => {
                // MOVW - YA := (imm)[16-bit]
                let addr = self.load();
                let value = self.read16_small(addr);
                let [a, y] = value.to_le_bytes();
                self.a = a;
                self.y = y;
                self.update_nz16(value);
            }
            0xbb => {
                // INC - (imm + X)++
                let addr = self.load().wrapping_add(self.x);
                let addr = self.get_small(addr);
                let val = self.read(addr).wrapping_add(1);
                self.write(addr, val);
                self.update_nz8(val);
            }
            0xbc => {
                // INC - A
                self.a = self.a.wrapping_add(1);
                self.update_nz8(self.a);
            }
            0xbd => {
                // MOV - SP := X
                self.sp = self.x
            }
            0xbe => {
                // DAS - Decimal adjust after subtraction
                if self.a & 0xf0 >= 10 || self.status & flags::CARRY == 0 {
                    self.a -= 0x60;
                    self.status &= !flags::CARRY
                }
                if self.a & 15 >= 10 || self.status & flags::HALF_CARRY == 0 {
                    self.a -= 6;
                }
                self.update_nz8(self.a)
            }
            0xbf => {
                // MOV - A := (X++)
                self.a = self.read_small(self.x);
                self.x = self.x.wrapping_add(1);
                self.update_nz8(self.a)
            }
            0xc0 => {
                // DI - Clear INTERRUPT_ENABLE
                self.status &= !flags::INTERRUPT_ENABLE
            }
            0xc4 => {
                // MOV - (db) := A
                let addr = self.load();
                self.write_small(addr, self.a)
            }
            0xc5 => {
                // MOV - (imm[16-bit]) := A
                let addr = self.load16();
                self.write(addr, self.a)
            }
            0xc6 => {
                // MOV - (X) := A
                self.write_small(self.x, self.a)
            }
            0xc7 => {
                // MOV - ((imm+X)[16-bit]) := A
                let addr = self.load().wrapping_add(self.x);
                let addr = self.read16_small(addr);
                self.write(addr, self.a)
            }
            0xc8 => {
                // CMP - X - IMM
                let val = self.load();
                self.compare(self.x, val)
            }
            0xc9 => {
                // MOV - (imm[16-bit]) := X
                let addr = self.load16();
                self.write(addr, self.x)
            }
            0xca => {
                // MOV1 - (imm[13-bit])[bit] = C
                let addr = self.load16();
                let (shift, addr) = (addr >> 13, addr & 0x1fff);
                let val = self.read(addr) & !(1 << shift);
                self.write(addr, val | ((self.status & flags::CARRY) << shift));
            }
            0xcb => {
                // MOV - (imm) := Y
                let addr = self.load();
                self.write_small(addr, self.y)
            }
            0xcc => {
                // MOV - (imm[16-bit]) := Y
                let addr = self.load16();
                self.write(addr, self.y)
            }
            0xcd => {
                // MOV - X := IMM
                self.x = self.load();
                self.update_nz8(self.x);
            }
            0xce => {
                // POP - X
                self.x = self.pull()
            }
            0xcf => {
                // MUL - YA := Y * A
                self.set_ya(u16::from(self.y) * u16::from(self.a));
                self.update_nz8(self.y);
            }
            0xd0 => {
                // BNE/JNZ - if not Zero
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::ZERO == 0, &mut cycles)
            }
            0xd4 => {
                // MOV - (imm+X) := A
                let addr = self.load().wrapping_add(self.x);
                self.write_small(addr, self.a)
            }
            0xd5 => {
                // MOV - (imm[16-bit]+X) := A
                let addr = self.load16().wrapping_add(self.x.into());
                self.write(addr, self.a)
            }
            0xd6 => {
                // MOV - (imm[16-bit]+Y) := A
                let addr = self.load16().wrapping_add(self.y.into());
                self.write(addr, self.a)
            }
            0xd7 => {
                // MOV - ((db)[16-bit] + Y) := A
                let addr = self.load();
                let addr = self.read16_small(addr).wrapping_add(self.y.into());
                self.write(addr, self.a);
            }
            0xd8 => {
                // MOV - (imm) := X
                let addr = self.load();
                self.write_small(addr, self.x)
            }
            0xd9 => {
                // MOV - (imm) := X
                let addr = self.load().wrapping_add(self.y);
                self.write_small(addr, self.x)
            }
            0xda => {
                // MOVW - (imm)[16-bit] := YA
                // TODO: calculate cyles as if only one byte written
                let addr = self.load();
                self.write16_small(addr, u16::from_le_bytes([self.a, self.y]));
            }
            0xdb => {
                // MOV - (imm+X) := Y
                let addr = self.load().wrapping_add(self.x);
                self.write_small(addr, self.y)
            }
            0xdc => {
                // DEC - Y
                self.y = self.y.wrapping_sub(1);
                self.update_nz8(self.y);
            }
            0xdd => {
                // MOV - A := Y
                self.a = self.y;
                self.update_nz8(self.a)
            }
            0xde => {
                // CBNE - Branch if A != (imm+X)
                let addr = self.load().wrapping_add(self.x);
                let val = self.read_small(addr);
                let rel = self.load();
                self.branch_rel(rel, self.a != val, &mut cycles)
            }
            0xdf => {
                // DAA - Decimal adjust after addition
                if self.a & 0xf0 >= 10 || self.status & flags::CARRY > 0 {
                    self.a -= 0xa0;
                    self.status |= flags::CARRY
                }
                if self.a & 15 >= 10 || self.status & flags::HALF_CARRY > 0 {
                    self.a -= 10;
                }
                self.update_nz8(self.a)
            }
            0xe4 => {
                // MOV - A := (imm)
                let addr = self.load();
                self.a = self.read_small(addr);
                self.update_nz8(self.a);
            }
            0xe5 => {
                // MOV - A := (imm[16-bit])
                let addr = self.load16();
                self.a = self.read(addr);
                self.update_nz8(self.a);
            }
            0xe8 => {
                // MOV - A := IMM
                self.a = self.load();
                self.update_nz8(self.a);
            }
            0xe9 => {
                // MOV - X := (imm[16-bit])
                let addr = self.load16();
                self.x = self.read(addr);
                self.update_nz8(self.x);
            }
            0xea => {
                // NOT1 - Complement Bit in Memory address
                let imm = self.load16();
                let addr = imm & 0x1fff;
                let val = self.read(addr) ^ (1u8 << (imm >> 13));
                self.write(addr, val)
            }
            0xeb => {
                // MOV - Y := (IMM)
                let addr = self.load();
                self.y = self.read_small(addr);
                self.update_nz8(self.y)
            }
            0xe0 => {
                // CLRV - Clear OVERFLOW and HALF_CARRY
                self.status &= !(flags::OVERFLOW | flags::HALF_CARRY)
            }
            0xe6 => {
                // MOV - A := (X)
                self.a = self.read_small(self.x);
                self.update_nz8(self.a)
            }
            0xe7 => {
                // MOV - A := ((imm[16-bit]+X)[16-bit])
                let addr = self.load().wrapping_add(self.x);
                self.a = self.read(self.read16_small(addr));
                self.update_nz8(self.a);
            }
            0xec => {
                // MOV - Y := (imm[16-bit])
                let addr = self.load16();
                self.y = self.read(addr);
                self.update_nz8(self.y);
            }
            0xed => {
                // NOTC - Complement CARRY
                self.status ^= flags::CARRY
            }
            0xee => {
                // POP - Y
                self.y = self.pull()
            }
            0xf0 => {
                // BEQ - Branch if ZERO is set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::ZERO > 0, &mut cycles)
            }
            0xf4 => {
                // MOV - A := (imm+X)
                let addr = self.load().wrapping_add(self.x);
                self.a = self.read_small(addr);
                self.update_nz8(self.a);
            }
            0xf5 => {
                // MOV - A := (imm[16-bit]+X)
                let addr = self.load16().wrapping_add(self.x.into());
                self.a = self.read(addr);
                self.update_nz8(self.a);
            }
            0xf6 => {
                // MOV - A := (imm[16-bit]+Y)
                let addr = self.load16().wrapping_add(self.y.into());
                self.a = self.read(addr);
                self.update_nz8(self.a);
            }
            0xf7 => {
                // MOV - A := ((imm)[16-bit]+Y)
                let addr = self.load();
                let addr = self.read16_small(addr).wrapping_add(self.y.into());
                self.a = self.read(addr);
                self.update_nz8(self.a);
            }
            0xf8 => {
                // MOV - X := (imm)
                let addr = self.load();
                self.x = self.read_small(addr);
                self.update_nz8(self.x);
            }
            0xf9 => {
                // MOV - X := (imm+Y)
                let addr = self.load().wrapping_add(self.y);
                self.x = self.read_small(addr);
                self.update_nz8(self.x);
            }
            0xfa => {
                // MOV - (dp) := (dp)
                let val1 = self.load();
                let val1 = self.read_small(val1);
                let val2 = self.load();
                self.write_small(val2, val1);
            }
            0xfb => {
                // MOV - Y := (imm+X)
                let addr = self.load().wrapping_add(self.x);
                self.y = self.read_small(addr);
                self.update_nz8(self.y);
            }
            0xfc => {
                // INC - Y
                self.y = self.y.wrapping_add(1);
                self.update_nz8(self.y);
            }
            0xfd => {
                // MOV - Y := A
                self.y = self.a;
                self.update_nz8(self.y)
            }
            0xfe => {
                // DBNZ - Y--; JNZ
                self.y = self.y.wrapping_sub(1);
                let rel = self.load();
                self.branch_rel(rel, self.y > 0, &mut cycles)
            }
            0xef | 0xff => {
                // SLEEP / STOP - Halt the processor
                self.halt = true
            }
        }
        cycles
    }

    pub fn update_nz8(&mut self, val: u8) {
        if val > 0 {
            self.status = (self.status & !(flags::ZERO | flags::SIGN)) | (val & flags::SIGN);
        } else {
            self.status = (self.status & !flags::SIGN) | flags::ZERO
        }
    }

    pub fn update_nz16(&mut self, val: u16) {
        if val > 0 {
            self.status =
                (self.status & !(flags::ZERO | flags::SIGN)) | ((val >> 8) as u8 & flags::SIGN);
        } else {
            self.status = (self.status & !flags::SIGN) | flags::ZERO
        }
    }

    pub fn branch_rel(&mut self, rel: u8, cond: bool, cycles: &mut Cycles) {
        if cond {
            if rel < 0x80 {
                self.pc = self.pc.wrapping_add(rel.into());
            } else {
                self.pc = self.pc.wrapping_sub(0x100 - u16::from(rel));
            }
            *cycles += 2;
        }
    }

    pub fn compare(&mut self, a: u8, b: u8) {
        let res = a as u16 + !b as u16 + 1;
        self.set_status(res > 0xff, flags::CARRY);
        self.update_nz8((res & 0xff) as u8);
    }

    pub fn adc(&mut self, a: u8, b: u8) -> u8 {
        let c = self.status & flags::CARRY;
        let (res, ov1) = a.overflowing_add(b);
        let (res, ov2) = res.overflowing_add(c);
        self.set_status(
            (a & 0x80 == b & 0x80) && (b & 0x80 != res & 0x80),
            flags::OVERFLOW,
        );
        self.set_status(((a & 15) + (b & 15) + c) > 15, flags::HALF_CARRY);
        self.set_status(ov1 || ov2, flags::CARRY);
        self.update_nz8(res);
        res
    }

    pub fn add16(&mut self, a: u16, b: u16) -> u16 {
        let (res, ov) = a.overflowing_add(b);
        self.set_status(
            (a & 0x8000 == b & 0x8000) && (b & 0x8000 != res & 0x8000),
            flags::OVERFLOW,
        );
        self.set_status(((a & 0xfff) + (b & 0xfff)) > 0xffe, flags::HALF_CARRY);
        self.set_status(ov, flags::CARRY);
        self.update_nz16(res);
        res
    }

    pub fn adc16(&mut self, a: u16, b: u16) -> u16 {
        let c = u16::from(self.status & flags::CARRY);
        let (res, ov1) = a.overflowing_add(b);
        let (res, ov2) = res.overflowing_add(c);
        self.set_status(
            (a & 0x8000 == b & 0x8000) && (b & 0x8000 != res & 0x8000),
            flags::OVERFLOW,
        );
        self.set_status(((a & 0xfff) + (b & 0xfff) + c) > 0xfff, flags::HALF_CARRY);
        self.set_status(ov1 || ov2, flags::CARRY);
        self.update_nz16(res);
        res
    }

    pub fn update_timer(&mut self, i: usize) {
        if self.timer_enable & (1 << i) > 0 {
            self.timers[i] = self.timers[i].wrapping_add(1);
            if self.timers[i] == self.timer_max[i] {
                self.timers[i] = 0;
                self.counters[i].set(self.counters[i].get().wrapping_add(1) & 0xf);
            }
        }
    }

    pub fn run_cycle(&mut self) -> Option<StereoSample> {
        if self.cycles_ahead == 0 && !self.halt {
            self.cycles_ahead = self.dispatch_instruction();
        }
        self.cycles_ahead = self.cycles_ahead.saturating_sub(1);
        self.dsp.run_one_step(&mut self.mem);
        let mut output = None;
        if self.dispatch_counter & 0xf == 0 {
            if self.dispatch_counter & 0x1f == 0 {
                output = Some(self.dsp.global_output);
                if self.dispatch_counter & 0x7f == 0 {
                    self.update_timer(0);
                    self.update_timer(1);
                }
            }
            self.update_timer(2);
        }
        self.dispatch_counter = self.dispatch_counter.wrapping_add(1);
        output
    }
}
