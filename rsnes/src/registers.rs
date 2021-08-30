use crate::device::Device;

impl Device {
    pub fn read_internal_register(&self, id: u16) -> Option<u8> {
        match id {
            0x4300..=0x43ff => {
                // DMA Registers
                self.dma.read(id)
            }
            _ => todo!("internal register 0x{:04x} read", id),
        }
    }

    pub fn write_internal_register(&mut self, id: u16, val: u8) {
        match id {
            0x4016 => {
                // JOYSER0 - NES-style Joypad access
                self.cpu.latch_line = val & 1 > 0;
            }
            0x4200 => {
                // NMITIMEN - Interrupt Enable Flags
                // TODO: implement expected behavior
                self.cpu.nmitimen = val
            }
            0x4201 => {
                // WRIO - Programmable I/O-Port
                if self.cpu.pio & 0x80 > 0 && val & 0x80 == 0 {
                    // TODO: latch ppu counters
                }
                self.cpu.pio = val;
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
