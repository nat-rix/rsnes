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
pub(crate) const APU_CPU_TIMING_PROPORTION_NTSC: (Cycles, Cycles) = (118125, 5632);
pub(crate) const APU_CPU_TIMING_PROPORTION_PAL: (Cycles, Cycles) = (665, 32);

impl<B: crate::backend::AudioBackend, FB: crate::backend::FrameBuffer> Device<B, FB> {
    pub fn run_cycle<const N: u16>(&mut self) {
        self.smp.tick(N);
        let vend = self.vend();
        if self.is_auto_joypad() && self.new_scanline && self.ppu.scanline_nr == vend + 2 {
            self.controllers.auto_joypad_timer = 4224;
            self.controllers.auto_joypad()
        }
        self.controllers.auto_joypad_timer -= self.controllers.auto_joypad_timer.min(N);
        // > The CPU is paused for 40 cycles beginning about 536 cycles
        // > after the start of each scanline
        // source: <https://wiki.superfamicom.org/timing>
        if !(536..536 + 40).contains(&self.ppu.scanline_cycle) {
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
        if self.do_hdma && self.ppu.scanline_nr < vend && self.ppu.scanline_cycle >= 1024 {
            self.do_hdma = false;
            self.dma.hdma_ahead_cycles = self.do_hdma();
        }
        if self.new_scanline && self.ppu.scanline_nr == vend {
            self.ppu.vblank();
        }
        if !self.scanline_drawn
            && self.ppu.scanline_nr + 1 < vend
            && self.ppu.scanline_cycle + crate::ppu::RAY_AHEAD_CYCLES >= self.get_line_length()
        {
            self.scanline_drawn = true;
            self.ppu.draw_line(self.ppu.scanline_nr + 1)
        }
        let h_irq_enabled = self.cpu.nmitimen & 0x10 > 0;
        let v_irq_enabled = self.cpu.nmitimen & 0x20 > 0;
        self.shall_irq = self.shall_irq
            || ((h_irq_enabled || v_irq_enabled)
                && (!h_irq_enabled
                    || ((self.ppu.scanline_cycle as i16 - N as i16) >> 2
                        < self.irq_time_h as i16
                        && self.ppu.scanline_cycle >> 2 >= self.irq_time_h))
                && (!v_irq_enabled || self.ppu.scanline_nr == self.irq_time_v)
                && (h_irq_enabled || !v_irq_enabled || self.new_scanline));
        let do_nmi = self.new_scanline && self.ppu.scanline_nr == vend;
        self.nmi_vblank_bit.set(self.nmi_vblank_bit.get() || do_nmi);
        self.shall_nmi = self.cpu.nmitimen & 0x80 > 0 && (self.shall_nmi || do_nmi);
        self.update_counters::<N>();
    }

    pub fn update_counters<const N: u16>(&mut self) {
        self.ppu.scanline_cycle += N;
        self.math_registers.tick(N);
        self.new_scanline = false;
        self.new_frame = false;
        let line_length = self.get_line_length();
        // Test if one scanline completed
        // TODO: Take notice of the interlace mode
        if self.ppu.scanline_cycle >= line_length {
            self.ppu.scanline_cycle -= line_length;
            self.ppu.scanline_nr += 1;
            self.do_hdma = true;
            self.new_scanline = true;
            self.scanline_drawn = false;
            let scanline_count = self.scanline_count();
            // Test if one frame completed
            // TODO: Take notice of the interlace mode
            if self.ppu.scanline_nr >= scanline_count {
                self.ppu.scanline_nr -= scanline_count;
                self.new_frame = true;
                self.nmi_vblank_bit.set(false);
                self.ppu.field ^= true;
                self.smp.refresh();
            } else if self.smp.is_threaded() {
                // if the S-SMP is threaded, refresh it every scanline
                self.smp.refresh();
            }
        }
    }

    pub fn get_line_length(&self) -> u16 {
        if !self.is_pal && !self.ppu.interlace && self.ppu.field && self.ppu.scanline_nr == 240 {
            1360
        } else if self.is_pal && self.ppu.interlace && self.ppu.field && self.ppu.scanline_nr == 311
        {
            1368
        } else {
            1364
        }
    }

    pub fn run_cpu<const N: u16>(&mut self) {
        let needs_refresh = self.cpu_ahead_cycles <= 0;
        self.cpu_ahead_cycles -= i32::from(N);
        if needs_refresh {
            self.memory_cycles = 0;
            let cycles = (if self.shall_nmi {
                if replace(&mut self.cpu.wait_mode, false) {
                    return;
                }
                self.shall_nmi = false;
                self.nmi()
            } else if self.shall_irq {
                if replace(&mut self.cpu.wait_mode, false) {
                    return;
                }
                self.shall_irq = false;
                self.irq()
            } else {
                if self.cpu.wait_mode {
                    return;
                }
                // > Internal operation CPU cycles always take 6 master cycles
                // source: <https://wiki.superfamicom.org/memory-mapping>
                self.dispatch_instruction() * 6
            }) + self.memory_cycles;
            self.cpu_ahead_cycles += cycles as i32;
        }
    }

    pub fn vend(&self) -> u16 {
        (if self.ppu.overscan {
            crate::ppu::MAX_SCREEN_HEIGHT_OVERSCAN
        } else {
            crate::ppu::MAX_SCREEN_HEIGHT
        } + 1) as _
    }

    pub fn scanline_count(&self) -> u16 {
        if self.is_pal {
            312
        } else {
            262
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
        macro_rules! romaccess {
            () => {
                if self.cpu.access_speed {
                    Fast
                } else {
                    Slow
                }
            };
        }
        (match addr.bank {
            0x00..=0x3f => match addr.addr {
                0x0000..=0x1fff => Slow,
                0x2000..=0x3fff => Fast,
                0x4000..=0x41ff => XSlow,
                0x4200..=0x5fff => Fast,
                0x6000..=0xffff => Slow,
            },
            0x40..=0x7f => Slow,
            0x80..=0xbf => match addr.addr {
                0x0000..=0x1fff => Slow,
                0x2000..=0x3fff => Fast,
                0x4000..=0x41ff => XSlow,
                0x4200..=0x5fff => Fast,
                0x6000..=0x7fff => Slow,
                0x8000..=0xffff => romaccess!(),
            },
            0xc0..=0xff => romaccess!(),
        }) as u8 as Cycles
    }
}
