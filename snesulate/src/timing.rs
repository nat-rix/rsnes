use crate::device::{Addr24, Device};

pub type Cycles = u32;

impl Device {
    pub fn run_cycle(&mut self) {
        // > Internal operation CPU cycles always take 6 master cycles
        // source: <https://wiki.superfamicom.org/memory-mapping>
        let cycles = self.dispatch_instruction() * 6 + self.memory_cycles;
        self.master_cycle += u64::from(cycles);
        self.memory_cycles = 0;
        println!("master cycle {}", self.master_cycle);
    }

    pub fn get_memory_cycle(&self, addr: Addr24) -> Cycles {
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
