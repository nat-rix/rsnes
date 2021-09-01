#[derive(Debug, Clone)]
pub struct Oam {
    low: [u8; 512],
    high: [u8; 32],
    is_high: bool,
    addr: u8,
    addr_inc: u8,
    priority: bool,
}

impl Oam {
    pub fn new() -> Self {
        Self {
            low: [0; 512],
            high: [0; 32],
            is_high: false,
            addr: 0,
            addr_inc: 0,
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
        self.addr = value;
        self.addr_inc = value;
    }

    pub fn set_addr_high(&mut self, value: u8) {
        self.is_high = value & 1 > 0;
        self.addr_inc = self.addr;
        self.priority = value & 0x80 > 0;
    }
}
