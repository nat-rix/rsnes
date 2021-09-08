//! SPC700 Sound Coprocessor handling types
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/spc700-reference>
//! - <https://emudev.de/q00-snes/spc700-the-audio-processor/>
//! - The first of the two official SNES documentation books

use crate::timing::{Cycles, CPU_64KHZ_TIMING_PROPORTION as TIMING_PROPORTION};
use core::cell::Cell;

pub const MEMORY_SIZE: usize = 64 * 1024;

static ROM: [u8; 64] = [
    0xCD, 0xEF, 0xBD, 0xE8, 0x00, 0xC6, 0x1D, 0xD0, 0xFC, 0x8F, 0xAA, 0xF4, 0x8F, 0xBB, 0xF5, 0x78,
    0xCC, 0xF4, 0xD0, 0xFB, 0x2F, 0x19, 0xEB, 0xF4, 0xD0, 0xFC, 0x7E, 0xF4, 0xD0, 0x0B, 0xE4, 0xF5,
    0xCB, 0xF4, 0xD7, 0x00, 0xFC, 0xD0, 0xF3, 0xAB, 0x01, 0x10, 0xEF, 0x7E, 0xF4, 0x10, 0xEB, 0xBA,
    0xF6, 0xDA, 0x00, 0xBA, 0xF4, 0xC4, 0xF4, 0xDD, 0x5D, 0xD0, 0xDB, 0x1F, 0x00, 0x00, 0xC0, 0xFF,
];

/// Noise clock frequencies in Hz
const FREQUENCIES: [u16; 32] = [
    0, 16, 21, 25, 31, 42, 50, 63, 83, 100, 125, 167, 200, 250, 333, 400, 500, 667, 800, 1000,
    1300, 1600, 2000, 2700, 3200, 4000, 5300, 6400, 8000, 10700, 16000, 32000,
];

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

const F1_RESET: u8 = 0xb0;

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

#[derive(Debug, Clone, Copy)]
pub struct Channel {}

impl Channel {
    pub const fn new() -> Self {
        Self {}
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
    // FLG register (6c)
    flags: u8,
    master_volume: [i8; 2],
    echo_volume: [i8; 2],
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
            flags: 0,
            master_volume: [0, 0],
            echo_volume: [0, 0],
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
    counters: [Cell<u8>; 3],
}

impl Spc700 {
    pub const fn new() -> Self {
        const fn generate_power_up_memory() -> [u8; MEMORY_SIZE] {
            let mut mem: [u8; MEMORY_SIZE] =
                unsafe { core::mem::transmute([[[0x00u8; 32], [0xffu8; 32]]; 1024]) };
            mem[0xf1] = F1_RESET;
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
            counters: [Cell::new(0), Cell::new(0), Cell::new(0)],
        }
    }

    pub fn reset(&mut self) {
        self.mem[0xf1] = F1_RESET;
        self.input = [0; 4];
        self.output = [0; 4];
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.sp = 0;
        // actually self.read16(0xfffe), but this will
        // always result in 0xffc0, because mem[0xf1] = 0xb0
        self.pc = 0xffc0;
        self.status = 0;
    }

    pub const fn is_rom_mapped(&self) -> bool {
        self.mem[0xf1] & 0x80 > 0
    }

    pub fn read16(&self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read(addr), self.read(addr.wrapping_add(1))])
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xf3 => self.read_dsp_register(self.mem[0xf2]),
            0xf4..=0xf7 => self.input[usize::from(addr - 0xf4)],
            0xfd..=0xff => self.counters[usize::from(addr - 0xfd)].take(),
            0xf0..=0xf1 | 0xf8..=0xff => {
                todo!("reading SPC register 0x{:02x}", addr)
            }
            0xffc0..=0xffff if self.is_rom_mapped() => ROM[(addr & 0x3f) as usize],
            addr => self.mem[addr as usize],
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xf1 => {
                // TODO: Reset active timers & activate
                if val & 0x10 > 0 {
                    self.input[0..2].fill(0)
                }
                if val & 0x20 > 0 {
                    self.input[2..4].fill(0)
                }
            }
            0xf3 => self.write_dsp_register(self.mem[0xf2], val),
            0xf4..=0xf7 => self.output[(addr - 0xf4) as usize] = val,
            0xfa | 0xfb => self.timer_max[usize::from(addr & 1)] = u16::from(val) << 3,
            0xfc => self.timer_max[2] = val.into(),
            0xf0 | 0xf8..=0xff => {
                todo!("writing 0x{:02x} to SPC register 0x{:02x}", val, addr)
            }
            addr => self.mem[addr as usize] = val,
        }
    }

    pub fn read_dsp_register(&self, id: u8) -> u8 {
        match id {
            0x0c => self.dsp.master_volume[0] as u8,
            0x1c => self.dsp.master_volume[1] as u8,
            0x2c => self.dsp.echo_volume[0] as u8,
            0x3c => self.dsp.echo_volume[1] as u8,
            0x6c => self.dsp.flags,

            0x0d => self.dsp.echo_feedback as u8,
            0x2d => self.dsp.pitch_modulation,
            0x3d => self.dsp.noise,
            0x4d => self.dsp.echo,
            0x5d => (self.dsp.source_dir_addr >> 8) as u8,
            0x6d => (self.dsp.echo_data_addr >> 8) as u8,
            0x7d => self.dsp.echo_delay >> 4,

            _ => todo!("read dsp register 0x{:02x}", id),
        }
    }

    pub fn write_dsp_register(&mut self, id: u8, val: u8) {
        match id {
            0x0c => self.dsp.master_volume[0] = val as i8,
            0x1c => self.dsp.master_volume[1] = val as i8,
            0x2c => self.dsp.echo_volume[0] = val as i8,
            0x3c => self.dsp.echo_volume[1] = val as i8,
            0x6c => self.dsp.flags = val,

            0x0d => self.dsp.echo_feedback = val as i8,
            0x2d => self.dsp.pitch_modulation = val & 0xfe,
            0x3d => self.dsp.noise = val,
            0x4d => self.dsp.echo = val,
            0x5d => self.dsp.source_dir_addr = u16::from(val) << 8,
            0x6d => self.dsp.echo_data_addr = u16::from(val) << 8,
            0x7d => self.dsp.echo_delay = val << 4,

            _ => todo!("write value 0x{:02x} dsp register 0x{:02x}", val, id),
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
            self.timers[i] = self.timers[i].wrapping_add(div.into());
            let div = self.timers[i].checked_div(self.timer_max[i]).unwrap_or(0);
            self.timers[i] = self.timers[i].checked_rem(self.timer_max[i]).unwrap_or(0);
            self.counters[i].set(self.counters[i].get().wrapping_add((div & 0xff) as u8) & 0xf);
        }
    }
}
