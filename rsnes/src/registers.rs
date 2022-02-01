use crate::device::Device;
use save_state_macro::*;

const CHIP_5A22_VERSION: u8 = 2;

#[derive(Debug, Clone, InSaveState)]
pub struct MathRegisters {
    multiplicands: [u8; 2],
    dividend: u16,
    divisor: u8,
    math_timer: u16,
    result_after: [u8; 4],
    result_before: [u8; 4],
}

impl MathRegisters {
    pub const fn new() -> Self {
        Self {
            multiplicands: [0xff, 0xff],
            dividend: 0xffff,
            divisor: 0xff,
            math_timer: 0,
            result_after: [0; 4],
            result_before: [0; 4],
        }
    }

    pub fn tick(&mut self, cycles: u16) {
        self.math_timer = self.math_timer.saturating_sub(cycles)
    }

    pub fn fire_multiply(&mut self) {
        if self.math_timer == 0 {
            self.result_before = self.result_after
        }
        let [lower, higher] =
            (u16::from(self.multiplicands[0]) * u16::from(self.multiplicands[1])).to_le_bytes();
        self.math_timer = 48;
        self.result_after = [self.multiplicands[1], 0, lower, higher]
    }

    pub fn fire_divide(&mut self) {
        if self.math_timer == 0 {
            self.result_before = self.result_after
        }
        let (div, rem) = if self.divisor == 0 {
            (0xffff, self.dividend)
        } else {
            (
                self.dividend / u16::from(self.divisor),
                self.dividend % u16::from(self.divisor),
            )
        };
        let [div_low, div_high] = div.to_le_bytes();
        let [rem_low, rem_high] = rem.to_le_bytes();
        self.math_timer = 96;
        self.result_after = [div_low, div_high, rem_low, rem_high]
    }

    pub const fn get_result(&self) -> &[u8; 4] {
        if self.math_timer == 0 {
            &self.result_after
        } else {
            &self.result_before
        }
    }
}

impl<B: crate::backend::AudioBackend, FB: crate::backend::FrameBuffer> Device<B, FB> {
    pub fn read_internal_register(&mut self, id: u16) -> Option<u8> {
        match id {
            0x4016 => {
                // JOYSER0 - NES-style Joypad access
                Some(self.controllers.port1.read_port_data() | (self.open_bus & 0xfc))
            }
            0x4017 => {
                // JOYSER1 - NES-style Joypad access
                Some(self.controllers.port2.read_port_data() | 0b11100 | (self.open_bus & 0xfc))
            }
            0x4210 => {
                // NMI Flag & CPU version
                // TODO: check if version 2 is appropriate
                Some(
                    ((self.nmi_vblank_bit.replace(false) as u8) << 7)
                        | CHIP_5A22_VERSION
                        | (self.open_bus & 0x70),
                )
            }
            0x4211 => {
                // TIMEUP - The IRQ flag
                self.shall_irq = false;
                Some(self.irq_bit.take() | (self.open_bus & 0x7f))
            }
            0x4212 => {
                // HVBJOY - PPU status
                // TODO: better timing and auto joypad timing
                let in_hblank = self.ppu.scanline_cycle >= 1096 || self.ppu.scanline_cycle <= 2;
                Some(
                    (((self.ppu.scanline_nr >= self.vend()) as u8) << 7)
                        | ((in_hblank as u8) << 6)
                        | (self.controllers.auto_joypad_timer > 0) as u8
                        | (self.open_bus & 0x3e),
                )
            }
            0x4214..=0x4217 => {
                // Math result registers
                Some(self.math_registers.get_result()[usize::from(id & 3)])
            }
            0x4218..=0x421f => {
                // JOYnL/JOYnH
                Some(self.controllers.access(id))
            }
            0x4300..=0x43ff => {
                // DMA Registers
                self.dma.read(id)
            }
            0x4200..=0x420f => None,
            _ => todo!("internal register 0x{:04x} read", id),
        }
    }

    pub fn is_auto_joypad(&self) -> bool {
        self.cpu.nmitimen & 1 > 0
    }

    pub fn write_internal_register(&mut self, id: u16, val: u8) {
        match id {
            0x4016 => {
                // JOYSER0 - NES-style Joypad access
                self.controllers.set_strobe(val & 1 > 0)
            }
            0x4200 => {
                // NMITIMEN - Interrupt Enable Flags
                // TODO: implement expected behavior
                self.cpu.nmitimen = val;
            }
            0x4201 => {
                // WRIO - Programmable I/O-Port
                if self.controllers.set_pio(val) {
                    self.ppu.latch()
                }
            }
            0x4202 => {
                // WRMPYA
                self.math_registers.multiplicands[0] = val
            }
            0x4203 => {
                // WRMPYB
                self.math_registers.multiplicands[1] = val;
                self.math_registers.fire_multiply()
            }
            0x4204 => {
                // WRDIVL
                self.math_registers.dividend =
                    (self.math_registers.dividend & 0xff00) | u16::from(val)
            }
            0x4205 => {
                // WRDIVH
                self.math_registers.dividend =
                    (self.math_registers.dividend & 0xff) | (u16::from(val) << 8)
            }
            0x4206 => {
                // WRDIVB
                self.math_registers.divisor = val;
                self.math_registers.fire_divide()
            }
            0x4207 => {
                // HTIMEL
                self.irq_time_h = (self.irq_time_h & 0x100) | u16::from(val)
            }
            0x4208 => {
                // HTIMEH
                self.irq_time_h = (self.irq_time_h & 0xff) | (u16::from(val & 1) << 8)
            }
            0x4209 => {
                // VTIMEL
                self.irq_time_v = (self.irq_time_v & 0x100) | u16::from(val)
            }
            0x420a => {
                // VTIMEH
                self.irq_time_v = (self.irq_time_v & 0xff) | (u16::from(val & 1) << 8)
            }
            0x420b => {
                // MDMAEN - DMA Enable
                // TODO: implement expected behavior
                self.dma.enable_dma(val)
            }
            0x420c => {
                // HDMAEN - HDMA Enable
                // TODO: implement expected behavior
                self.dma.enable_hdma(val)
            }
            0x420d => {
                // MEMSEL - ROM access speed
                self.cpu.access_speed = val & 1 > 0
            }
            0x4300..=0x43ff => {
                // DMA Registers
                self.dma.write(id, val)
            }
            _ => todo!("internal register 0x{:04x} written", id),
        }
    }
}
