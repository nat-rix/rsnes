use core::{cell::Cell, mem::replace};

pub mod buttons {
    pub const B: u16 = 1;
    pub const Y: u16 = 2;
    pub const SELECT: u16 = 4;
    pub const START: u16 = 8;
    pub const UP: u16 = 0x10;
    pub const DOWN: u16 = 0x20;
    pub const LEFT: u16 = 0x40;
    pub const RIGHT: u16 = 0x80;
    pub const A: u16 = 0x100;
    pub const X: u16 = 0x200;
    pub const L: u16 = 0x400;
    pub const R: u16 = 0x800;
}

#[derive(Debug, Clone)]
pub enum Controller {
    None,
    Standard(StandardController),
}

impl Controller {
    pub fn poll_bit_data1(&self) -> bool {
        match self {
            Self::None => false,
            Self::Standard(StandardController { shift_register, .. }) => {
                shift_register.get() & 1 > 0
            }
        }
    }

    pub fn poll_bit_data2(&self) -> bool {
        false
    }

    pub fn on_strobe(&mut self) {
        match self {
            Self::Standard(cntrl) => cntrl.shift_register.set(cntrl.pressed_buttons),
            Self::None => (),
        }
    }

    pub fn on_clock(&self) {
        match self {
            Self::None => (),
            Self::Standard(StandardController { shift_register, .. }) => {
                shift_register.set((shift_register.get() >> 1) | 0x8000)
            }
        }
    }
}

/// The standard SNES-Controller with A,B,X,Y,Left,Right,Up,Down,
/// L,R,Start,Select buttons
#[derive(Debug, Clone)]
pub struct StandardController {
    shift_register: Cell<u16>,
    pressed_buttons: u16,
}

impl StandardController {
    pub const fn new() -> Self {
        Self {
            shift_register: Cell::new(0),
            pressed_buttons: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ControllerPort {
    controller: Controller,
    pio: bool,
    strobe: bool,
    data1: u16,
    data2: u16,
}

impl ControllerPort {
    pub const fn new(controller: Controller) -> Self {
        Self {
            controller,
            pio: false,
            strobe: false,
            data1: 0,
            data2: 0,
        }
    }

    pub fn set_strobe(&mut self, bit: bool) {
        if !replace(&mut self.strobe, bit) && bit {
            self.controller.on_strobe()
        }
    }

    pub fn read_port_data(&self) -> u8 {
        let bit1 = self.controller.poll_bit_data1();
        let bit2 = self.controller.poll_bit_data2();
        self.controller.on_clock();
        (bit1 as u8) | ((bit2 as u8) << 1)
    }
}

#[derive(Debug, Clone)]
pub struct ControllerPorts {
    pub port1: ControllerPort,
    pub port2: ControllerPort,
    pub(crate) auto_joypad_timer: u16,
}

impl ControllerPorts {
    pub fn new() -> Self {
        Self {
            port1: ControllerPort::new(Controller::Standard(StandardController::new())),
            port2: ControllerPort::new(Controller::None),
            auto_joypad_timer: 0,
        }
    }

    /// Write to the programmable I/O-port.
    /// Returns if EXTLATCH shall be triggered.
    pub fn set_pio(&mut self, val: u8) -> bool {
        self.port1.pio = val & 0x40 > 0;
        let pio2 = val & 0x80 > 0;
        !replace(&mut self.port2.pio, pio2) && pio2
    }

    pub fn set_strobe(&mut self, bit: bool) {
        self.port1.set_strobe(bit);
        self.port2.set_strobe(bit);
    }

    pub fn auto_joypad(&mut self) {
        for port in [&mut self.port1, &mut self.port2] {
            port.set_strobe(false);
            port.set_strobe(true);
            port.data1 = 0;
            port.data2 = 0;
            for _ in 0..16 {
                port.data1 <<= 1;
                port.data2 <<= 1;
                let data = port.read_port_data();
                port.data1 |= u16::from(data & 1);
                port.data2 |= u16::from(data >> 1);
            }
        }
    }

    pub(crate) fn access(&self, id: u16) -> u8 {
        let port = if id & 2 > 0 { &self.port2 } else { &self.port1 };
        let data = if id & 4 > 0 { port.data2 } else { port.data1 };
        if id & 1 > 0 {
            (data >> 8) as u8
        } else {
            (data & 0xff) as u8
        }
    }
}
