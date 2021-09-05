use crate::device::Device;

impl Device {
    pub fn read_internal_register(&self, id: u16) -> Option<u8> {
        match id {
            0x4210 => {
                // NMI Flag & CPU version
                // TODO: check if version 2 is appropriate
                let version = 2;
                Some(((self.nmi_vblank_bit.replace(false) as u8) << 7) | version)
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
                    "autojoipad is now {}",
                    ["disabled", "enabled"][self.is_auto_joypad() as usize]
                );
            }
            0x4201 => {
                // WRIO - Programmable I/O-Port
                if self.controllers.set_pio(val) {
                    // TODO: latch ppu counters
                }
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
