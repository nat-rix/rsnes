use core::{cell::Cell, mem::replace};
use save_state_macro::*;

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
    Mouse(Mouse),
}

impl Controller {
    pub fn poll_bit_data1(&self) -> bool {
        match self {
            Self::None => false,
            Self::Standard(StandardController { shift_register, .. }) => {
                shift_register.get() & 1 > 0
            }
            Self::Mouse(Mouse { shift_register, .. }) => shift_register.get() & 1 > 0,
        }
    }

    pub fn poll_bit_data2(&self) -> bool {
        match self {
            Self::None | Self::Standard(_) | Self::Mouse(_) => false,
        }
    }

    pub fn on_strobe(&mut self) {
        match self {
            Self::Standard(cntrl) => cntrl.shift_register.set(cntrl.pressed_buttons),
            Self::Mouse(mouse) => {
                let [dx, dy] = mouse.internal_offset.map(|i| i.clamp(-0x7f, 0x7f));
                mouse.internal_offset[0] = mouse.internal_offset[0].wrapping_sub(dx);
                mouse.internal_offset[1] = mouse.internal_offset[1].wrapping_sub(dy);
                let [dx, dy] =
                    [dx, dy].map(|v| ((v.abs() as u8).reverse_bits() << 1) | (v < 0) as u8);
                mouse.shift_register.set(
                    0x8000
                        | ((mouse.right_button as u32) << 8)
                        | ((mouse.left_button as u32) << 9)
                        | ((mouse.speed as u32) << 10)
                        | ((dy as u32) << 16)
                        | ((dx as u32) << 24),
                );
            }
            Self::None => (),
        }
    }

    pub fn on_clock(&self) {
        match self {
            Self::None => (),
            Self::Standard(StandardController { shift_register, .. }) => {
                shift_register.set((shift_register.get() >> 1) | 0x8000)
            }
            Self::Mouse(Mouse { shift_register, .. }) => {
                shift_register.set((shift_register.get() >> 1) | 0x8000_0000)
            }
        }
    }

    pub fn on_strobe_clock(&mut self) {
        match self {
            Self::Mouse(mouse) => {
                mouse.speed += 1;
                if mouse.speed >= 3 {
                    mouse.speed = 0;
                }
            }
            _ => (),
        }
    }
}

impl save_state::InSaveState for Controller {
    fn serialize(&self, state: &mut save_state::SaveStateSerializer) {
        let n: u8 = match self {
            Self::None => 0,
            Self::Standard(..) => 1,
            Self::Mouse(..) => 2,
        };
        n.serialize(state);
        match self {
            Self::None => (),
            Self::Standard(v) => v.serialize(state),
            Self::Mouse(v) => v.serialize(state),
        }
    }

    fn deserialize(&mut self, state: &mut save_state::SaveStateDeserializer) {
        let mut n: u8 = 0;
        n.deserialize(state);
        *self = match n {
            0 => Self::None,
            1 => {
                let mut cntrl = StandardController::default();
                cntrl.deserialize(state);
                Self::Standard(cntrl)
            }
            2 => {
                let mut mouse = Mouse::default();
                mouse.deserialize(state);
                Self::Mouse(mouse)
            }
            _ => panic!("unexpected discriminant value {}", n),
        }
    }
}

#[derive(Debug, Clone, Default, InSaveState)]
pub struct Mouse {
    shift_register: Cell<u32>,
    speed: u8,
    pub left_button: bool,
    pub right_button: bool,
    pub internal_offset: [i32; 2],
}

impl Mouse {
    pub fn add_offset(&mut self, off: [i32; 2]) {
        for (i, c) in off.into_iter().enumerate() {
            let c = match self.speed {
                1 => c.saturating_add(c >> 1),
                2 => c << 1,
                _ => c,
            };
            self.internal_offset[i] = self.internal_offset[i].saturating_add(c);
        }
    }
}

/// The standard SNES-Controller with A,B,X,Y,Left,Right,Up,Down,
/// L,R,Start,Select buttons
#[derive(Debug, Default, Clone, InSaveState)]
pub struct StandardController {
    shift_register: Cell<u16>,
    pub pressed_buttons: u16,
}

impl StandardController {
    pub const fn new() -> Self {
        Self {
            shift_register: Cell::new(0),
            pressed_buttons: 0,
        }
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct ControllerPort {
    pub controller: Controller,
    strobe: bool,
    data1: u16,
    data2: u16,
}

impl ControllerPort {
    pub const fn new(controller: Controller) -> Self {
        Self {
            controller,
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

    pub fn read_port_data(&mut self) -> u8 {
        let bit1 = self.controller.poll_bit_data1();
        let bit2 = self.controller.poll_bit_data2();
        if !self.strobe {
            self.controller.on_strobe_clock();
        }
        self.controller.on_clock();
        (bit1 as u8) | ((bit2 as u8) << 1)
    }
}

#[derive(Debug, Clone, InSaveState)]
pub struct ControllerPorts {
    pub port1: ControllerPort,
    pub port2: ControllerPort,
    pio: u8,
    pub(crate) auto_joypad_timer: u16,
}

impl ControllerPorts {
    pub fn new() -> Self {
        Self {
            port1: ControllerPort::new(Controller::Standard(StandardController::new())),
            port2: ControllerPort::new(Controller::None),
            pio: 0,
            auto_joypad_timer: 0,
        }
    }

    /// Write to the programmable I/O-port.
    /// Returns if EXTLATCH shall be triggered.
    pub fn set_pio(&mut self, val: u8) -> bool {
        (replace(&mut self.pio, val) & !val) & 0x80 > 0
    }

    pub const fn get_pio(&self) -> u8 {
        self.pio
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
