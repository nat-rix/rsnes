use save_state_macro::*;

#[derive(Debug, Clone, Copy, InSaveState)]
pub struct Object {
    pub x: i16,
    pub y: u8,
    pub tile_nr: u8,
    pub attrs: u8,
    pub is_large: bool,
}

impl Object {
    pub const fn new() -> Object {
        Self {
            x: 0,
            y: 0,
            tile_nr: 0,
            attrs: 0,
            is_large: false,
        }
    }

    pub fn write_low_low(&mut self, val1: u8, val2: u8) {
        self.x = (((self.x as u16) & 0x100) | u16::from(val1)) as i16;
        self.y = val2;
    }

    pub fn write_low_high(&mut self, val1: u8, val2: u8) {
        self.tile_nr = val1;
        self.attrs = val2;
    }

    pub fn write_high(&mut self, val: u8) {
        self.x = ((self.x as u16 & 0xff) | (u16::from(val & 1) << 8)) as i16;
        self.is_large = val & 2 > 0;
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct Oam {
    pub(crate) objs: [Object; 128],
    // 10-bit value
    pub(crate) addr: u16,
    pub(crate) addr_inc: u16,
    stashed_write: u8,
    pub(crate) priority: bool,
}

impl Oam {
    pub const fn new() -> Self {
        Self {
            objs: [Object::new(); 128],
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
        self.addr = (self.addr & 0x1fe) | (u16::from(value & 1) << 9);
        self.addr_inc = self.addr;
        self.priority = value & 0x80 > 0;
    }

    pub fn write(&mut self, value: u8) {
        let addr = self.addr_inc;
        if addr & 1 == 0 {
            self.stashed_write = value
        }
        self.addr_inc = self.addr_inc.wrapping_add(1);
        if addr > 0x1ff {
            let i = usize::from((addr & 31) << 2);
            self.objs[i].write_high(value & 3);
            self.objs[i | 1].write_high((value >> 2) & 3);
            self.objs[i | 2].write_high((value >> 4) & 3);
            self.objs[i | 3].write_high(value >> 6);
        } else if addr & 1 == 1 {
            [Object::write_low_low, Object::write_low_high][usize::from((addr >> 1) & 1)](
                &mut self.objs[usize::from(addr >> 2)],
                self.stashed_write,
                value,
            );
        }
    }
}

#[derive(Debug, Clone, InSaveState)]
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

    pub fn read16(&self, addr: u8) -> u16 {
        let addr = usize::from(addr) << 1;
        u16::from_le_bytes([self.data[addr], self.data[addr | 1]])
    }
}
