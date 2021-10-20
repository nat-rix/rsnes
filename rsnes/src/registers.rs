use crate::device::Device;

const CHIP_5A22_VERSION: u8 = 2;

impl Device {
    pub fn read_internal_register(&self, id: u16) -> Option<u8> {
        match id {
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
                Some(self.irq_bit.take() | (self.open_bus & 0x7f))
            }
            0x4218..=0x421f => {
                // JOYnL/JOYnH
                Some(self.controllers.access(id))
            }
            0x4300..=0x43ff => {
                // DMA Registers
                self.dma.read(id)
            }
            _ => todo!("internal register 0x{:04x} read", id),
        }
    }

    pub const fn is_auto_joypad(&self) -> bool {
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
                println!(
                    "autojoypad is now {}",
                    ["disabled", "enabled"][self.is_auto_joypad() as usize]
                );
            }
            0x4201 => {
                // WRIO - Programmable I/O-Port
                if self.controllers.set_pio(val) {
                    // TODO: latch ppu counters
                }
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
                println!("writing to id {:04x}", id);
                self.dma.write(id, val)
            }
            _ => todo!("internal register 0x{:04x} written", id),
        }
    }
}
