#[derive(Debug, Clone)]
pub struct Oam {
    low: [u8; 512],
    high: [u8; 32],
    // 10-bit value
    addr: u16,
    addr_inc: u16,
    stashed_write: u8,
    priority: bool,
}

impl Oam {
    pub const fn new() -> Self {
        Self {
            low: [0; 512],
            high: [0; 32],
            addr: 0,
            addr_inc: 0,
            stashed_write: 0,
            priority: false,
        }
    }

    /// Reset the OAM address.
    /// This occurs usually at the beginning of a V-Blank
    /// if it is not fblanked.
    pub fn oam_reset(&mut self) {
        self.addr_inc = self.addr
    }

    pub fn set_addr_low(&mut self, value: u8) {
        self.addr = u16::from(value) << 1;
        self.addr_inc = self.addr;
    }

    pub fn set_addr_high(&mut self, value: u8) {
        self.addr = (self.addr & 0x1ff) | (u16::from(value & 1) << 9);
        self.addr_inc = self.addr;
        self.priority = value & 0x80 > 0;
    }

    pub fn read(&mut self) -> u8 {
        let value = if self.addr > 0x1ff {
            self.high[usize::from(self.addr_inc & 0x1f)]
        } else {
            self.low[usize::from(self.addr_inc & 0x1ff)]
        };
        self.addr_inc = self.addr_inc.wrapping_add(1);
        value
    }

    pub fn write(&mut self, value: u8) {
        if self.addr > 0x1ff {
            self.high[usize::from(self.addr_inc & 0x1f)] = value
        } else {
            if self.addr_inc & 1 == 0 {
                self.stashed_write = value
            } else {
                self.low[usize::from(self.addr_inc & 0x1fe)] = self.stashed_write;
                self.low[usize::from(self.addr_inc & 0x1ff)] = value;
            }
        }
        self.addr_inc = self.addr_inc.wrapping_add(1);
    }
}

#[derive(Debug, Clone)]
pub struct CgRam {
    data: [u8; 512],
    // 9-bit value
    addr: u16,
    stashed_write: u8,
}

impl CgRam {
    pub const fn new() -> Self {
        Self {
            data: [0; 512],
            addr: 0,
            stashed_write: 0,
        }
    }

    pub fn set_addr(&mut self, addr: u8) {
        self.addr = u16::from(addr) << 1;
    }

    pub fn write(&mut self, value: u8) {
        if self.addr & 1 == 0 {
            self.stashed_write = value
        } else {
            self.data[usize::from(self.addr & 0x1fe)] = self.stashed_write;
            self.data[usize::from(self.addr & 0x1ff)] = value
        }
        self.addr = self.addr.wrapping_add(1)
    }
}
