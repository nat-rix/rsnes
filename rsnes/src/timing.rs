//! Timing control implementation
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/timing>

use crate::device::{Addr24, Device};

pub type Cycles = u32;

// The SNES master clock runs at ca. (945/44) MHz which is ca. 21.477Hz;
// The APU runs at 1024MHz

/// This is a fractional proportion between the cpu and apu clock speed
const APU_CPU_TIMING_PROPORTION: (Cycles, Cycles) = (118125, 5632);

impl Device {
    pub fn run_cycle<const N: u16>(&mut self) {
        // > The CPU is paused for 40 cycles beginning about 536 cycles
        // > after the start of each scanline
        // source: <https://wiki.superfamicom.org/timing>
        if !(536..536 + 40).contains(&self.scanline_cycle) {
            if self.dma.is_dma_running() && !self.dma.is_hdma_running() {
                todo!("do dma stuff")
            } else {
                self.run_cpu();
            }
        }
        self.cpu_ahead_cycles -= i32::from(N);
        self.scanline_cycle += N;
        // Test if one scanline completed
        // TODO: Take notice of the interlace mode
        if self.scanline_cycle >= 1364 {
            self.scanline_cycle -= 1364;
            self.scanline_nr += 1;
            // Test if one frame completed
            // TODO: Take notice of the interlace mode
            if self.scanline_nr >= 262 {
                self.scanline_nr -= 262;
            }
        }
    }

    pub fn run_cpu(&mut self) {
        while self.cpu_ahead_cycles <= 0 {
            self.memory_cycles = 0;
            // > Internal operation CPU cycles always take 6 master cycles
            // source: <https://wiki.superfamicom.org/memory-mapping>
            let cycles = self.dispatch_instruction() * 6 + self.memory_cycles;
            self.master_cycle += cycles;
            self.cpu_ahead_cycles += cycles as i32;
            while self.apu_cycles * APU_CPU_TIMING_PROPORTION.0
                < self.master_cycle * APU_CPU_TIMING_PROPORTION.1
            {
                self.apu_cycles += self.spc.dispatch_instruction();
                while self.master_cycle >= APU_CPU_TIMING_PROPORTION.0
                    && self.apu_cycles >= APU_CPU_TIMING_PROPORTION.1
                {
                    self.master_cycle -= APU_CPU_TIMING_PROPORTION.0;
                    self.apu_cycles -= APU_CPU_TIMING_PROPORTION.1;
                }
            }
        }
    }

    pub const fn get_memory_cycle(&self, addr: Addr24) -> Cycles {
        #[repr(u8)]
        enum Speed {
            Fast = 6,
            Slow = 8,
            XSlow = 12,
        }
        use Speed::*;
        const fn romaccess(device: &Device) -> Speed {
            if device.cpu.access_speed {
                Fast
            } else {
                Slow
            }
        }
        (match addr.bank {
            0x00..=0x3f => match addr.addr {
                0x0000..=0x1fff => Slow,
                0x2000..=0x20ff => Fast,
                0x2100..=0x21ff => Fast,
                0x2200..=0x3fff => Fast,
                0x4000..=0x41ff => XSlow,
                0x4200..=0x43ff => Fast,
                0x4400..=0x5fff => Fast,
                0x6000..=0x7fff => Slow,
                0x8000..=0xffff => Slow,
            },
            0x40..=0x7f => Slow,
            0x80..=0xbf => match addr.addr {
                0x0000..=0x1fff => Slow,
                0x2000..=0x20ff => Fast,
                0x2100..=0x21ff => Fast,
                0x2200..=0x3fff => Fast,
                0x4000..=0x41ff => XSlow,
                0x4200..=0x43ff => Fast,
                0x4400..=0x5fff => Fast,
                0x6000..=0x7fff => Slow,
                0x8000..=0xffff => romaccess(self),
            },
            0xc0..=0xff => romaccess(self),
        }) as u8 as Cycles
    }
}
