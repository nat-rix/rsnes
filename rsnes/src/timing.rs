//! Timing control implementation
//!
//! # Literature
//!
//! - <https://wiki.superfamicom.org/timing>

use crate::{
    cpu::Status,
    device::{Addr24, Device},
};

pub type Cycles = u32;

// The SNES master clock runs at ca. (945/44) MHz which is ca. 21_477kHz;
// The APU runs at 1024kHz

/// This is a fractional proportion between the cpu and apu clock speed
pub(crate) const APU_CPU_TIMING_PROPORTION_NTSC: (Cycles, Cycles) = (118125, 5632);
pub(crate) const APU_CPU_TIMING_PROPORTION_PAL: (Cycles, Cycles) = (665, 32);

pub(crate) const NECDSP_CPU_TIMING_PROPORTION_NTSC: (Cycles, Cycles) = (118125, 45056);
pub(crate) const NECDSP_CPU_TIMING_PROPORTION_PAL: (Cycles, Cycles) = (40591, 15625);

impl<B: crate::backend::AudioBackend, FB: crate::backend::FrameBuffer> Device<B, FB> {
    pub fn run_cycle<const N: u16>(&mut self) {
        self.smp.tick(N);
        self.cartridge.as_mut().unwrap().tick(N.into());
        let vend = self.ppu.vend();
        if self.is_auto_joypad() && self.new_scanline && self.ppu.get_pos().y == vend + 2 {
            self.controllers.auto_joypad_timer = 4224;
            self.controllers.auto_joypad()
        }
        self.controllers.auto_joypad_timer -= self.controllers.auto_joypad_timer.min(N);
        // > The CPU is paused for 40 cycles beginning about 536 cycles
        // > after the start of each scanline
        // source: <https://wiki.superfamicom.org/timing>
        if self.ppu.is_cpu_active() && self.cpu.active {
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
        if self.cartridge.as_ref().unwrap().has_sa1() {
            self.with_sa1_cpu().run_cpu::<N>();
        }
        if self.new_frame {
            self.dma.hdma_ahead_cycles = self.reset_hdma();
        }
        if self.do_hdma && !self.ppu.is_in_vblank() && self.ppu.get_pos().x >= 1024 {
            self.do_hdma = false;
            self.dma.hdma_ahead_cycles = self.do_hdma();
        }
        let vblanked = self.new_scanline && self.ppu.get_pos().y == vend;
        if vblanked {
            self.ppu.vblank();
        }
        if self.ppu.get_pos().x + crate::ppu::RAY_AHEAD_CYCLES >= self.ppu.get_scanline_cycles()
            && self.ppu.get_pos().y + 1 < vend
            && !self.scanline_drawn
        {
            self.scanline_drawn = true;
            self.ppu.draw_scanline();
        }
        let h_irq_enabled = self.cpu.nmitimen & 0x10 > 0;
        let v_irq_enabled = self.cpu.nmitimen & 0x20 > 0;
        self.shall_irq = self.shall_irq
            || ((h_irq_enabled || v_irq_enabled)
                && (!h_irq_enabled
                    || ((self.ppu.get_pos().x as i16 - N as i16) >> 2 < self.irq_time_h as i16
                        && self.ppu.get_pos().x >> 2 >= self.irq_time_h))
                && (!v_irq_enabled || self.ppu.get_pos().y == self.irq_time_v)
                && (h_irq_enabled || !v_irq_enabled || self.new_scanline));
        self.nmi_vblank_bit
            .set(self.nmi_vblank_bit.get() || vblanked);
        self.shall_nmi = self.cpu.nmitimen & 0x80 > 0 && (self.shall_nmi || vblanked);
        self.update_counters::<N>();
    }

    pub fn update_counters<const N: u16>(&mut self) {
        self.ppu.mut_pos().x += N;
        self.math_registers.tick(N);
        self.new_scanline = false;
        self.new_frame = false;
        let line_length = self.ppu.get_scanline_cycles();
        // Test if one scanline completed
        // TODO: Take notice of the interlace mode
        if self.ppu.get_pos().x >= line_length {
            self.ppu.mut_pos().x -= line_length;
            self.ppu.mut_pos().y += 1;
            self.do_hdma = true;
            self.new_scanline = true;
            self.scanline_drawn = false;
            let scanline_count = self.ppu.get_scanline_count();
            // Test if one frame completed
            // TODO: Take notice of the interlace mode
            if self.ppu.get_pos().y >= scanline_count {
                self.ppu.mut_pos().y -= scanline_count;
                self.new_frame = true;
                self.nmi_vblank_bit.set(false);
                self.ppu.end_vblank();
                self.smp.refresh();
                self.cartridge.as_mut().unwrap().refresh_coprocessors();
            } else if self.smp.is_threaded() {
                // if the S-SMP is threaded, refresh it every scanline
                self.smp.refresh();
            }
        }
    }

    pub fn run_cpu<const N: u16>(&mut self) {
        let needs_refresh = self.cpu_ahead_cycles <= 0;
        self.cpu_ahead_cycles -= i32::from(N);
        if needs_refresh {
            // > WAI/HALT stops the CPU until an exception (usually an IRQ or NMI) request occurs
            // > in case of IRQs this works even if IRQs are disabled (via I=1).
            // source: FullSNES
            if self.cpu.wait_mode {
                self.cpu.wait_mode = !self.shall_nmi && !self.shall_irq;
                self.cpu_ahead_cycles += 1;
                return;
            }
            self.memory_cycles = 0;
            let cycles = (if self.shall_nmi {
                self.shall_nmi = false;
                self.with_main_cpu().nmi()
            } else if (self.shall_irq || self.get_irq_pin())
                && !self.cpu.regs.status.has(Status::IRQ_DISABLE)
            {
                self.shall_irq = false;
                self.with_main_cpu().irq()
            } else {
                // > Internal operation CPU cycles always take 6 master cycles
                // source: <https://wiki.superfamicom.org/memory-mapping>
                self.with_main_cpu().dispatch_instruction() * 6
            }) + self.memory_cycles;
            self.cpu_ahead_cycles += cycles as i32;
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
