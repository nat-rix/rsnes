//! Timing control implementation
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/timing>

use crate::device::{Addr24, Device};
use core::mem::replace;

pub type Cycles = u32;

// The SNES master clock runs at ca. (945/44) MHz which is ca. 21_477kHz;
// The APU runs at 1024kHz

/// This is a fractional proportion between the cpu and apu clock speed
pub(crate) const APU_CPU_TIMING_PROPORTION: (Cycles, Cycles) = (118125, 5632);

impl<B: crate::backend::AudioBackend, FB: crate::backend::FrameBuffer> Device<B, FB> {
    pub fn run_cycle<const N: u16>(&mut self) {
        self.spc.tick(N);
        let vend = self.vend();
        if self.new_scanline && self.scanline_nr < vend {
            self.ppu.draw_line(self.scanline_nr)
        }
        if self.is_auto_joypad() && self.new_scanline && self.scanline_nr == vend + 2 {
            self.controllers.auto_joypad_timer = 4224;
            self.controllers.auto_joypad()
        }
        self.controllers.auto_joypad_timer -= self.controllers.auto_joypad_timer.min(N);
        // > The CPU is paused for 40 cycles beginning about 536 cycles
        // > after the start of each scanline
        // source: <https://wiki.superfamicom.org/timing>
        if !(536..536 + 40).contains(&self.scanline_cycle) {
            if self.dma.hdma_ahead_cycles > 0 {
                self.dma.hdma_ahead_cycles -= i32::from(N);
            } else if self.dma.is_dma_running() {
                if self.dma.ahead_cycles > 0 {
                    self.dma.ahead_cycles -= i32::from(N)
                } else {
                    self.do_dma_first_channel()
                }
            } else {
                self.run_cpu::<N>();
            }
        }
        if self.new_frame {
            self.dma.hdma_ahead_cycles = self.reset_hdma();
        }
        if self.do_hdma && self.scanline_nr < vend && self.scanline_cycle >= 1024 {
            self.do_hdma = false;
            self.dma.hdma_ahead_cycles = self.do_hdma();
        }
        if self.new_scanline && self.scanline_nr == vend {
            self.ppu.vblank();
        }
        let h_irq_enabled = self.cpu.nmitimen & 0x10 > 0;
        let v_irq_enabled = self.cpu.nmitimen & 0x20 > 0;
        let hpos_start = self.scanline_cycle >> 2;
        let hpos_end = hpos_start + ((N + 3) >> 2);
        self.shall_irq = self.shall_irq
            || ((h_irq_enabled || v_irq_enabled)
                && (!h_irq_enabled || (hpos_start..hpos_end).contains(&self.irq_time_h))
                && (!v_irq_enabled || self.scanline_nr == self.irq_time_v)
                && (h_irq_enabled || !v_irq_enabled || self.new_scanline));
        let do_nmi = self.new_scanline && self.scanline_nr == vend;
        self.nmi_vblank_bit.set(self.nmi_vblank_bit.get() || do_nmi);
        self.shall_nmi |= self.cpu.nmitimen & 0x80 > 0 && do_nmi;
        self.update_counters::<N>();
    }

    pub fn update_counters<const N: u16>(&mut self) {
        self.scanline_cycle += N;
        self.math_registers.tick(N);
        self.new_scanline = false;
        self.new_frame = false;
        // Test if one scanline completed
        // TODO: Take notice of the interlace mode
        if self.scanline_cycle >= 1364 {
            self.scanline_cycle -= 1364;
            self.scanline_nr += 1;
            self.do_hdma = true;
            self.new_scanline = true;
            // Test if one frame completed
            // TODO: Take notice of the interlace mode
            if self.scanline_nr >= 262 {
                self.scanline_nr -= 262;
                self.new_frame = true;
                self.spc.refresh();
            }
        }
    }

    pub fn run_cpu<const N: u16>(&mut self) {
        while self.cpu_ahead_cycles <= 0 {
            self.memory_cycles = 0;
            let cycles = (if self.shall_nmi {
                if replace(&mut self.cpu.wait_mode, false) {
                    break;
                }
                self.shall_nmi = false;
                self.nmi()
            } else if self.shall_irq {
                if replace(&mut self.cpu.wait_mode, false) {
                    break;
                }
                self.shall_irq = false;
                self.irq()
            } else {
                if self.cpu.wait_mode {
                    break;
                }
                // > Internal operation CPU cycles always take 6 master cycles
                // source: <https://wiki.superfamicom.org/memory-mapping>
                self.dispatch_instruction() * 6
            }) + self.memory_cycles;
            self.cpu_ahead_cycles += cycles as i32;
        }
        self.cpu_ahead_cycles -= i32::from(N);
    }

    pub fn vend(&self) -> u16 {
        if self.ppu.overscan {
            0xf0
        } else {
            0xe1
        }
    }

    pub fn get_memory_cycle(&self, addr: Addr24) -> Cycles {
        #[repr(u8)]
        enum Speed {
            Fast = 6,
            Slow = 8,
            XSlow = 12,
        }
        use Speed::*;
        fn romaccess<B: crate::backend::AudioBackend, FB: crate::backend::FrameBuffer>(
            device: &Device<B, FB>,
        ) -> Speed {
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
