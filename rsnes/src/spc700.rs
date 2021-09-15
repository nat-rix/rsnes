//! SPC700 Sound Coprocessor handling types
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/spc700-reference>
//! - <https://emudev.de/q00-snes/spc700-the-audio-processor/>
//! - The first of the two official SNES documentation books

use crate::timing::{Cycles, CPU_64KHZ_TIMING_PROPORTION as TIMING_PROPORTION};
use core::{cell::Cell, mem::take};

pub const MEMORY_SIZE: usize = 64 * 1024;

static ROM: [u8; 64] = [
    0xCD, 0xEF, 0xBD, 0xE8, 0x00, 0xC6, 0x1D, 0xD0, 0xFC, 0x8F, 0xAA, 0xF4, 0x8F, 0xBB, 0xF5, 0x78,
    0xCC, 0xF4, 0xD0, 0xFB, 0x2F, 0x19, 0xEB, 0xF4, 0xD0, 0xFC, 0x7E, 0xF4, 0xD0, 0x0B, 0xE4, 0xF5,
    0xCB, 0xF4, 0xD7, 0x00, 0xFC, 0xD0, 0xF3, 0xAB, 0x01, 0x10, 0xEF, 0x7E, 0xF4, 0x10, 0xEB, 0xBA,
    0xF6, 0xDA, 0x00, 0xBA, 0xF4, 0xC4, 0xF4, 0xDD, 0x5D, 0xD0, 0xDB, 0x1F, 0x00, 0x00, 0xC0, 0xFF,
];

const GAUSS_INTERPOLATION_POINTS: [i32; 16 * 32] = [
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

/// Noise clock frequencies in Hz
const FREQUENCIES: [u16; 32] = [
    0, 16, 21, 25, 31, 42, 50, 63, 83, 100, 125, 167, 200, 250, 333, 400, 500, 667, 800, 1000,
    1300, 1600, 2000, 2700, 3200, 4000, 5300, 6400, 8000, 10700, 16000, 32000,
];

const fn calculate_gain_noise_rates() -> [u16; 32] {
    let mut rates = [0; 32];
    macro_rules! gen_rates {
        (t0, $n:expr) => {
            rates[$n] = if $n < 0x1a {
                let inv = 0x22 - $n;
                let x = inv / 3;
                let y = inv % 3;
                (1 << (x - 2)) * y + (1 << x)
            } else {
                0x20 - $n
            }
        };
        (t1, $off:expr) => {
            gen_rates!(t0, $off);
            gen_rates!(t0, $off + 1);
        };
        (t2, $off:expr) => {
            gen_rates!(t1, $off);
            gen_rates!(t1, $off + 2);
            gen_rates!(t1, $off + 4);
            gen_rates!(t1, $off + 6);
        };
        (t3, $off:expr) => {
            gen_rates!(t2, $off);
            gen_rates!(t2, $off + 8);
            gen_rates!(t2, $off + 16);
            gen_rates!(t2, $off + 24);
        };
    }
    gen_rates!(t3, 0);
    rates
}

const ADSR_GAIN_NOISE_RATES: [u16; 32] = calculate_gain_noise_rates();

const DECODE_BUFFER_SIZE: usize = 3 + 16;

#[rustfmt::skip]
static CYCLES: [Cycles; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       2, 0, 4, 0, 0, 0, 0, 0,   2, 6, 0, 0, 0, 4, 0, 0,  // 0^
       2, 0, 4, 0, 0, 0, 0, 0,   0, 0, 6, 0, 2, 2, 0, 6,  // 1^
       2, 0, 4, 0, 3, 0, 0, 0,   2, 0, 0, 0, 0, 4, 0, 4,  // 2^
       2, 0, 4, 0, 4, 0, 0, 0,   0, 0, 6, 0, 0, 2, 0, 8,  // 3^
       2, 0, 4, 0, 0, 0, 0, 0,   0, 0, 0, 4, 0, 4, 0, 0,  // 4^
       0, 0, 4, 0, 0, 0, 0, 0,   0, 0, 0, 5, 2, 2, 0, 3,  // 5^
       2, 0, 4, 0, 0, 4, 0, 2,   2, 0, 0, 0, 0, 4, 5, 5,  // 6^
       0, 0, 4, 0, 0, 5, 5, 0,   5, 0, 5, 0, 2, 2, 3, 0,  // 7^
       2, 0, 4, 0, 3, 0, 0, 0,   0, 0, 0, 4, 5, 2, 4, 5,  // 8^
       2, 0, 4, 0, 0, 0, 0, 0,   0, 0, 5, 0, 2, 2,12, 5,  // 9^
       3, 0, 4, 0, 0, 0, 0, 0,   2, 0, 0, 4, 5, 2, 4, 4,  // a^
       2, 0, 4, 0, 0, 0, 0, 0,   0, 0, 5, 0, 2, 2, 0, 4,  // b^
       3, 0, 4, 0, 4, 5, 4, 0,   2, 5, 0, 4, 5, 2, 4, 9,  // c^
       2, 0, 4, 0, 5, 6, 0, 7,   4, 0, 5, 5, 2, 2, 6, 0,  // d^
       2, 0, 4, 0, 3, 4, 3, 6,   2, 0, 0, 3, 4, 3, 4, 0,  // e^
       2, 0, 4, 0, 4, 5, 5, 0,   0, 0, 0, 0, 2, 2, 0, 0,  // f^
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
#[repr(usize)]
enum AdsrPeriod {
    Attack = 0,
    Decay = 1,
    Sustain = 2,
    Gain = 3,
    Release = 4,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StereoSample<T> {
    pub l: T,
    pub r: T,
}

impl<T> StereoSample<T> {
    pub const fn new2(l: T, r: T) -> Self {
        Self { l, r }
    }
}

impl<T: Copy> StereoSample<T> {
    pub fn new(val: T) -> Self {
        Self { l: val, r: val }
    }
}

impl StereoSample<i16> {
    pub fn saturating_add32(&self, val: StereoSample<i32>) -> Self {
        let clamped = val.clamp16();
        Self {
            l: self.l.saturating_add(clamped.l),
            r: self.r.saturating_add(clamped.r),
        }
    }
}

impl StereoSample<i32> {
    pub fn clamp16(self) -> StereoSample<i16> {
        StereoSample {
            l: self.l.clamp(-0x8000, 0x7fff) as i16,
            r: self.r.clamp(-0x8000, 0x7fff) as i16,
        }
    }
}

impl<T: Into<i32>> StereoSample<T> {
    pub fn to_i32(self) -> StereoSample<i32> {
        StereoSample {
            l: self.l.into(),
            r: self.r.into(),
        }
    }
}

impl core::ops::Mul for StereoSample<i32> {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self {
            l: self.l * other.l,
            r: self.r * other.r,
        }
    }
}

impl<R: Copy, T: core::ops::Shr<R>> core::ops::Shr<R> for StereoSample<T> {
    type Output = StereoSample<T::Output>;
    fn shr(self, rhs: R) -> Self::Output {
        StereoSample {
            l: self.l >> rhs,
            r: self.r >> rhs,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Channel {
    volume: StereoSample<i8>,
    // pitch (corresponds to `pitch * 125/8 Hz`)
    pitch: u16,
    source_number: u8,
    dir_addr: u16,
    data_addr: u16,
    adsr: [u8; 2],
    gain_mode: u8,
    gain: u16,
    vx_env: u8,
    vx_out: u8,
    sustain: u16,
    unused: [u8; 3],

    decode_buffer: [i16; DECODE_BUFFER_SIZE],
    last_sample: i16,
    pitch_counter: u16,
    period: AdsrPeriod,
    period_rate_map: [u16; 4],
    rate_index: u16,
    end_bit: bool,
    loop_bit: bool,
}

impl Channel {
    pub const fn new() -> Self {
        Self {
            volume: StereoSample::new2(0, 0),
            pitch: 0,
            source_number: 0,
            dir_addr: 0,
            data_addr: 0,
            adsr: [0; 2],
            gain_mode: 0,
            gain: 0,
            vx_env: 0,
            vx_out: 0,
            sustain: 0,
            unused: [0; 3],
            decode_buffer: [0; DECODE_BUFFER_SIZE],
            last_sample: 0,
            pitch_counter: 0,
            period: AdsrPeriod::Attack,
            period_rate_map: [0; 4],
            rate_index: 0,
            end_bit: false,
            loop_bit: false,
        }
    }

    pub fn update_gain(&mut self, rate: u16) {
        match self.period {
            AdsrPeriod::Attack => {
                self.gain = self
                    .gain
                    .saturating_add(if rate == 1 { 1024 } else { 32 })
                    .min(0x7ff);
                if self.gain > 0x7df {
                    self.period = AdsrPeriod::Decay
                }
            }
            AdsrPeriod::Decay | AdsrPeriod::Sustain => {
                self.gain = self
                    .gain
                    .saturating_sub((self.gain.saturating_sub(1) >> 8) + 1);
                if self.period == AdsrPeriod::Decay && self.gain < self.sustain {
                    self.period = AdsrPeriod::Sustain
                }
            }
            AdsrPeriod::Gain => todo!("gain mode"),
            AdsrPeriod::Release => panic!("`update_gain` must not be called in release mode"),
        }
    }

    pub fn reset(&mut self) {
        self.period = AdsrPeriod::Release;
        self.gain = 0;
    }
}

#[derive(Debug, Clone)]
pub struct Dsp {
    // in milliseconds
    echo_delay: u8,
    source_dir_addr: u16,
    echo_data_addr: u16,
    channels: [Channel; 8],
    pitch_modulation: u8,
    echo_feedback: i8,
    noise: u8,
    echo: u8,
    fade_in: u8,
    fade_out: u8,
    // FLG register (6c)
    flags: u8,
    master_volume: StereoSample<i8>,
    echo_volume: StereoSample<i8>,
    unused: u8,
}

impl Dsp {
    pub const fn new() -> Self {
        Self {
            echo_delay: 0,
            source_dir_addr: 0,
            echo_data_addr: 0,
            channels: [Channel::new(); 8],
            pitch_modulation: 0,
            echo_feedback: 0,
            noise: 0,
            echo: 0,
            fade_in: 0,
            fade_out: 0,
            flags: 0x80,
            master_volume: StereoSample::new2(0, 0),
            echo_volume: StereoSample::new2(0, 0),
            unused: 0,
        }
    }
}

#[derive(Debug, Clone)]
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

    cpu_time: Cycles,
    timer_max: [u16; 3],
    // internal timer ticks ALL in 64kHz
    timers: [u16; 3],
    timer_enable: u8,
    counters: [Cell<u8>; 3],
    dispatch_counter: u8,
}

impl Spc700 {
    pub const fn new() -> Self {
        const fn generate_power_up_memory() -> [u8; MEMORY_SIZE] {
            let mut mem: [u8; MEMORY_SIZE] =
                unsafe { core::mem::transmute([[[0x00u8; 32], [0xffu8; 32]]; 1024]) };
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
            sp: 0,
            pc: 0xffc0,
            status: 0,

            cpu_time: 0,
            timer_max: [0; 3],
            timers: [0; 3],
            timer_enable: 0,
            counters: [Cell::new(0), Cell::new(0), Cell::new(0)],
            dispatch_counter: 0,
        }
    }

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
    }

    pub const fn is_rom_mapped(&self) -> bool {
        self.mem[0xf0] & 0x80 > 0
    }

    pub fn read16(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read(addr), self.read(addr.wrapping_add(1))])
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xf3 => self.read_dsp_register(self.mem[0xf2]),
            0xf4..=0xf7 => self.input[usize::from(addr - 0xf4)],
            0xfd..=0xff => self.counters[usize::from(addr - 0xfd)].take(),
            0xf1 | 0xf8..=0xff => {
                todo!("reading SPC register 0x{:02x}", addr)
            }
            0xffc0..=0xffff if self.is_rom_mapped() => ROM[(addr & 0x3f) as usize],
            addr => self.mem[addr as usize],
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xf1 => {
                if val & 0x10 > 0 {
                    self.input[0..2].fill(0)
                }
                if val & 0x20 > 0 {
                    self.input[2..4].fill(0)
                }
                self.timer_enable = val & 7;
                for i in 0..3 {
                    if val & (1 << i) > 0 {
                        self.counters[i].set(0);
                        self.timers[i] = 0;
                    }
                }
            }
            0xf3 => self.write_dsp_register(self.mem[0xf2], val),
            0xf4..=0xf7 => self.output[(addr - 0xf4) as usize] = val,
            0xfa | 0xfb => self.timer_max[usize::from(addr & 1)] = u16::from(val) << 3,
            0xfc => self.timer_max[2] = val.into(),
            0xf8..=0xff => {
                todo!("writing 0x{:02x} to SPC register 0x{:02x}", val, addr)
            }
            addr => self.mem[addr as usize] = val,
        }
    }

    pub fn read_dsp_register(&self, id: u8) -> u8 {
        let rid = id & 0x8f;
        if rid < 0xa {
            let channel = &self.dsp.channels[usize::from(id >> 4)];
            match rid {
                0 => channel.volume.l as u8,
                1 => channel.volume.r as u8,
                2 => (channel.pitch & 0xff) as u8,
                3 => (channel.pitch >> 8) as u8,
                4 => channel.source_number,
                5 | 6 => channel.adsr[usize::from(!rid & 1)],
                7 => channel.gain_mode,
                8 => channel.vx_env,
                9 => channel.vx_out,
                10 => channel.unused[0],
                11 => channel.unused[1],
                14 => channel.unused[2],
                _ => todo!("read dsp register 0x{:02x}", id),
            }
        } else {
            match id {
                0x0c => self.dsp.master_volume.l as u8,
                0x1c => self.dsp.master_volume.r as u8,
                0x2c => self.dsp.echo_volume.l as u8,
                0x3c => self.dsp.echo_volume.r as u8,
                0x4c => self.dsp.fade_in,
                0x5c => self.dsp.fade_out,
                0x6c => self.dsp.flags,

                0x0d => self.dsp.echo_feedback as u8,
                0x1d => self.dsp.unused,
                0x2d => self.dsp.pitch_modulation,
                0x3d => self.dsp.noise,
                0x4d => self.dsp.echo,
                0x5d => (self.dsp.source_dir_addr >> 8) as u8,
                0x6d => (self.dsp.echo_data_addr >> 8) as u8,
                0x7d => self.dsp.echo_delay >> 4,

                _ => todo!("read dsp register 0x{:02x}", id),
            }
        }
    }

    pub fn write_dsp_register(&mut self, id: u8, val: u8) {
        let rid = id & 0x8f;
        if rid < 0xa {
            let channel = &mut self.dsp.channels[usize::from(id >> 4)];
            match rid {
                0 => channel.volume.l = val as i8,
                1 => channel.volume.r = val as i8,
                2 => channel.pitch = (channel.pitch & 0x3f00) | u16::from(val),
                3 => channel.pitch = (channel.pitch & 0xff) | (u16::from(val & 0x3f) << 8),
                4 => {
                    channel.source_number = val;
                    channel.dir_addr = self.dsp.source_dir_addr.wrapping_add(u16::from(val) << 2);
                }
                5 => {
                    channel.adsr[0] = val;
                    channel.period_rate_map[AdsrPeriod::Attack as usize] =
                        ADSR_GAIN_NOISE_RATES[usize::from(((val & 0xf) << 1) | 1)];
                    channel.period_rate_map[AdsrPeriod::Decay as usize] =
                        ADSR_GAIN_NOISE_RATES[usize::from(((val & 0x70) >> 3) | 0x10)];
                }
                6 => {
                    channel.adsr[1] = val;
                    channel.period_rate_map[AdsrPeriod::Sustain as usize] =
                        ADSR_GAIN_NOISE_RATES[usize::from(val & 0x1f)];
                    channel.sustain = (u16::from(val >> 5) + 1) * 0x100;
                }
                7 => channel.gain_mode = val,
                8 => channel.vx_env = val,
                9 => channel.vx_out = val,
                10 => channel.unused[0] = val,
                11 => channel.unused[1] = val,
                14 => channel.unused[2] = val,
                _ => todo!("read dsp register 0x{:02x}", id),
            }
        } else {
            match id {
                0x0c => self.dsp.master_volume.l = val as i8,
                0x1c => self.dsp.master_volume.r = val as i8,
                0x2c => self.dsp.echo_volume.l = val as i8,
                0x3c => self.dsp.echo_volume.r = val as i8,
                0x4c => self.dsp.fade_in = val,
                0x5c => self.dsp.fade_out = val,
                0x6c => self.dsp.flags = val,

                0x0d => self.dsp.echo_feedback = val as i8,
                0x1d => self.dsp.unused = val,
                0x2d => self.dsp.pitch_modulation = val & 0xfe,
                0x3d => self.dsp.noise = val,
                0x4d => self.dsp.echo = val,
                0x5d => {
                    self.dsp.source_dir_addr = u16::from(val) << 8;
                    for channel in &mut self.dsp.channels {
                        channel.dir_addr = self
                            .dsp
                            .source_dir_addr
                            .wrapping_add(u16::from(channel.source_number) << 2)
                    }
                }
                0x6d => self.dsp.echo_data_addr = u16::from(val) << 8,
                0x7d => self.dsp.echo_delay = val << 4,

                _ => todo!("write value 0x{:02x} dsp register 0x{:02x}", val, id),
            }
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

    pub fn sound_cycle(&mut self) {
        let (fade_in, fade_out) = if self.dispatch_counter & 0x3f == 0 {
            (take(&mut self.dsp.fade_in), self.dsp.fade_out)
        } else {
            (0, 0)
        };
        if self.dsp.flags & 0x80 > 0 {
            for channel in self.dsp.channels.iter_mut() {
                channel.reset()
            }
        }
        let mut last_sample = 0;
        let mut result = StereoSample::new(0i16);
        for (i, channel) in self.dsp.channels.iter_mut().enumerate() {
            if fade_out & (1 << i) > 0 {
                channel.period = AdsrPeriod::Release
            } else if fade_in & (1 << i) > 0 {
                channel.data_addr = u16::from_le_bytes([
                    self.mem[usize::from(channel.dir_addr)],
                    self.mem[usize::from(channel.dir_addr.wrapping_add(1))],
                ]);
                channel.loop_bit = false;
                channel.end_bit = false;
                channel.gain = 0;
                channel.period = if channel.adsr[0] & 0x80 > 0 {
                    AdsrPeriod::Attack
                } else {
                    AdsrPeriod::Gain
                };
            }
            let step = if self.dsp.pitch_modulation & (1 << i) > 0 && i != 0 {
                let factor = (last_sample >> 4) + 0x400;
                ((i32::from(channel.pitch) * i32::from(factor)) >> 10) as u16
            } else {
                channel.pitch as u16
            };
            let (new_pitch_counter, ov) = channel.pitch_counter.overflowing_add(step);
            channel.pitch_counter = new_pitch_counter;
            if ov {
                if channel.end_bit {
                    channel.data_addr = u16::from_le_bytes([
                        self.mem[usize::from(channel.dir_addr.wrapping_add(2))],
                        self.mem[usize::from(channel.dir_addr.wrapping_add(3))],
                    ]);
                    if !channel.loop_bit {
                        channel.reset()
                    }
                }
                channel
                    .decode_buffer
                    .copy_within(DECODE_BUFFER_SIZE - 3..DECODE_BUFFER_SIZE, 0);
                let header = self.mem[usize::from(channel.data_addr)];
                channel.end_bit = header & 1 > 0;
                channel.loop_bit = header & 2 > 0;
                channel.data_addr = channel.data_addr.wrapping_add(1);
                for byte_id in 0usize..8 {
                    let byte = self.mem[usize::from(channel.data_addr)];
                    channel.data_addr = channel.data_addr.wrapping_add(1);
                    let index = byte_id << 1;
                    use core::iter::once;
                    for (nibble_id, sample) in once(byte >> 4).chain(once(byte & 0xf)).enumerate() {
                        let index = index | nibble_id;
                        let sample = if sample & 8 > 0 {
                            (sample | 0xf0) as i8
                        } else {
                            sample as i8
                        };
                        let sample = match header >> 4 {
                            0 => i16::from(sample) >> 1,
                            s @ 1..=12 => i16::from(sample) << (s - 1),
                            13..=15 => todo!("what do values 13-15 mean? (stated as reserved)"),
                            _ => unreachable!(),
                        };
                        let older = channel.decode_buffer[index + 1];
                        let old = channel.decode_buffer[index + 2];
                        let sample = match header & 0b1100 {
                            0 => sample,
                            0b0100 => sample.saturating_add(old).saturating_add(-old >> 4),
                            0b1000 => sample
                                .saturating_add(old)
                                .saturating_add(old)
                                .saturating_add((-old).saturating_mul(3) >> 5)
                                .saturating_sub(older)
                                .saturating_add(older >> 4),
                            0b1100 => sample
                                .saturating_add(old)
                                .saturating_add(old)
                                .saturating_add((-old).saturating_mul(13) >> 6)
                                .saturating_sub(older)
                                .saturating_add(older.saturating_mul(3) >> 4),
                            _ => unreachable!(),
                        };
                        // this behaviour is documented by nocash FullSNES
                        let sample = sample.clamp(-0x8000, 0x7fff);
                        let sample = if sample > 0x3fff {
                            -0x8000 + sample
                        } else if sample < -0x4000 {
                            -0x8000 - sample
                        } else {
                            sample
                        };
                        channel.decode_buffer[index + 3] = sample
                    }
                }
            }
            let interpolation_index = (channel.pitch_counter >> 4) & 0xff;
            let brr_index = usize::from(channel.pitch_counter >> 12);
            let sample = (GAUSS_INTERPOLATION_POINTS[usize::from(0xff - interpolation_index)]
                * i32::from(channel.decode_buffer[brr_index]))
                >> 10;
            let sample = sample
                + ((GAUSS_INTERPOLATION_POINTS[usize::from(0x1ff - interpolation_index)]
                    * i32::from(channel.decode_buffer[brr_index + 1]))
                    >> 10);
            let sample = sample
                + ((GAUSS_INTERPOLATION_POINTS[usize::from(0x100 + interpolation_index)]
                    * i32::from(channel.decode_buffer[brr_index + 2]))
                    >> 10);
            let sample = sample & 0xffff;
            let sample = sample
                + ((GAUSS_INTERPOLATION_POINTS[usize::from(interpolation_index)]
                    * i32::from(channel.decode_buffer[brr_index + 3]))
                    >> 10);
            let sample = (sample.clamp(i16::MIN.into(), i16::MAX.into()) as i16) >> 1;
            if let AdsrPeriod::Release = channel.period {
                let (new_gain, ov) = channel.gain.overflowing_sub(8);
                channel.gain = if ov || new_gain > 0x7ff { 0 } else { new_gain };
            } else {
                // `channel.period as usize` will always be < 4
                let rate = channel.period_rate_map[channel.period as usize];
                if channel.gain_mode & 0x80 == 0 && channel.adsr[0] & 0x80 == 0 {
                    channel.gain = (channel.gain_mode & 0x7f).into()
                } else if rate > 0 {
                    channel.rate_index = channel.rate_index.wrapping_add(1);
                    if channel.rate_index >= rate {
                        channel.rate_index = 0;
                        channel.update_gain(rate)
                    }
                }
            }
            debug_assert!(channel.gain < 0x800);
            let sample = ((i32::from(sample) * i32::from(channel.gain)) >> 11) as i16;
            channel.last_sample = sample;
            last_sample = sample;
            channel.vx_env = (channel.gain >> 4) as u8; // TODO: really `>> 4`?
            channel.vx_out = (sample >> 7) as u8;
            result = result.saturating_add32(
                (StereoSample::new(i32::from(sample)) * channel.volume.to_i32()) >> 6,
            );
        }
        let result = if self.dsp.flags & 0x40 > 0 {
            StereoSample::new(0)
        } else {
            ((result.to_i32() * self.dsp.master_volume.to_i32()) >> 7).clamp16()
            // TODO: echo
            // TODO: noise
        };
        if result.l != 0 || result.r != 0 {
            println!("outputting sample {:?}", result);
        }
    }

    pub fn run_cycle(&mut self) -> Cycles {
        let cycles = self.dispatch_instruction();
        for _ in 0..cycles {
            if self.dispatch_counter & 0x1f == 0 {
                self.sound_cycle()
            }
            self.dispatch_counter = self.dispatch_counter.wrapping_add(1);
        }
        cycles
    }

    pub fn dispatch_instruction(&mut self) -> Cycles {
        let start_addr = self.pc;
        let op = self.load();
        println!("<SPC700> executing '{:02x}' @ ${:04x}", op, start_addr);
        let mut cycles = CYCLES[op as usize];
        match op {
            0x00 => (), // NOP
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
            0x08 => {
                // OR - A |= imm
                self.a |= self.load();
                self.update_nz8(self.a)
            }
            0x09 => {
                // OR - (imm) |= (imm)
                let (src, dst) = (self.load(), self.load());
                let dst = self.get_small(dst);
                self.write(dst, self.read_small(src) | self.read(dst))
            }
            0x0d => {
                // PUSH - status
                self.push(self.status)
            }
            0x10 => {
                // BPL/JNS - Branch if SIGN not set
                let rel = self.load();
                self.branch_rel(rel, self.status & flags::SIGN == 0, &mut cycles)
            }
            0x1a => {
                // DECW - (imm)[16-bit]--
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read16(addr).wrapping_sub(1);
                self.write16(addr, val);
                self.update_nz16(val)
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
            0x28 => {
                // AND - A &= imm
                self.a &= self.load();
                self.update_nz8(self.a)
            }
            0x2d => {
                // PUSH - A
                self.push(self.a)
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
            0x3a => {
                // INCW - (imm)[16-bit]++
                let addr = self.load();
                let addr = self.get_small(addr);
                let val = self.read16(addr).wrapping_add(1);
                self.write16(addr, val);
                self.update_nz16(val)
            }
            0x3d => {
                // INC - X
                self.x = self.x.wrapping_add(1);
                self.update_nz8(self.x);
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
            0x4d => {
                // PUSH - X
                self.push(self.x)
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
            0x65 => {
                // CMP - A - (imm[16-bit])
                let addr = self.load16();
                let val = self.read(addr);
                self.compare(self.a, val)
            }
            0x68 => {
                // CMP - A - imm
                let val = self.load();
                self.compare(self.a, val)
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
            0x78 => {
                // CMP - (imm) - imm
                let (b, a) = (self.load(), self.load());
                let a = self.read_small(a);
                self.compare(a, b)
            }
            0x7a => {
                // ADDW - YA += (imm)[16-bit]
                let addr = self.load();
                let val = self.read16_small(addr);
                let val = self.adc16(self.ya(), val);
                self.set_ya(val);
            }
            0x7c => {
                // ROR - A >>= 1
                self.set_status(self.a & 1 > 0, flags::CARRY);
                self.a = ((self.a & 0xfe) | (self.status & flags::CARRY)).rotate_right(1);
                self.update_nz8(self.a);
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
            0x9a => {
                // SUBW - YA -= (imm)[16-bit]
                let addr = self.load();
                let val = self.read16_small(addr);
                let val = self.adc16(self.ya(), !val);
                self.set_ya(val);
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
                self.update_nz8(self.a);
                self.a = (rdiv & 0xff) as u8;
                self.y = rmod;
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
            0xa8 => {
                // SBC - A -= imm + CARRY
                let val = self.load();
                self.a = self.adc(self.a, val);
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
            0xba => {
                // MOVW - YA := (imm)[16-bit]
                let addr = self.load();
                let value = self.read16_small(addr);
                let [a, y] = value.to_le_bytes();
                self.a = a;
                self.y = y;
                self.update_nz16(value);
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
            _ => todo!("not yet implemented SPC700 instruction 0x{:02x}", op),
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

    /// Tick in main CPU master cycles
    pub fn tick(&mut self, n: u16) {
        let delta = TIMING_PROPORTION.0.wrapping_mul(n.into());
        self.cpu_time = self.cpu_time.wrapping_add(delta);
        let div = self.cpu_time / TIMING_PROPORTION.1;
        self.cpu_time -= div * TIMING_PROPORTION.1;
        let div = (div & 0xff) as u8;
        for i in 0..3 {
            if self.timer_enable & (1 << i) > 0 {
                self.timers[i] = self.timers[i].wrapping_add(div.into());
                let div = self.timers[i].checked_div(self.timer_max[i]).unwrap_or(0);
                self.timers[i] = self.timers[i].checked_rem(self.timer_max[i]).unwrap_or(0);
                self.counters[i].set(self.counters[i].get().wrapping_add((div & 0xff) as u8) & 0xf);
            }
        }
    }
}
