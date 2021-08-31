#[derive(Debug, Clone)]
pub struct Ppu {
    /// A value between 0 and 15 with 15 being maximum brightness
    brightness: u8,
    obj_size: ObjectSize,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            brightness: 0x0f,
            obj_size: ObjectSize::O8S16,
        }
    }

    /// Read from a PPU register (memory map 0x2134..=0x213f)
    pub fn read_register(&self, id: u8) -> Option<u8> {
        todo!("read from unknown PPU register 0x21{:02x}", id)
    }

    /// Write to a PPU register (memory map 0x2100..=0x2133)
    pub fn write_register(&mut self, id: u8, val: u8) {
        match id {
            0x00 => {
                // INIDISP
                if val & 0b1000_0000 > 0 {
                    // TODO: force blank
                    println!("[warn] forcing blank (TODO)");
                }
                self.brightness = val & 0b1111;
                println!(
                    "setting brightness to {:.0}%",
                    (self.brightness as f32 * 100.0) / 15.0
                );
            }
            0x01 => {
                // OBSEL
                // TODO: name select bits and name base select bits
                self.obj_size = ObjectSize::from_upper_bits(val);
            }
            0x34.. => unreachable!(),
            _ => todo!("write to unknown PPU register 0x21{:02x}", id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectSize {
    /// Object 8x8 - Sprite 16x16
    O8S16,
    /// Object 8x8 - Sprite 32x32
    O8S32,
    /// Object 8x8 - Sprite 64x64
    O8S64,
    /// Object 16x16 - Sprite 32x32
    O16S32,
    /// Object 16x16 - Sprite 64x64
    O16S64,
    /// Object 32x32 - Sprite 64x64
    O32S64,
    /// Object 16x32 - Sprite 32x64
    O16x32S32x64,
    /// Object 16x32 - Sprite 32x32
    O16x32S32,
}

impl ObjectSize {
    pub fn from_upper_bits(bits: u8) -> Self {
        match bits >> 5 {
            0b000 => Self::O8S16,
            0b001 => Self::O8S32,
            0b010 => Self::O8S64,
            0b011 => Self::O16S32,
            0b100 => Self::O16S64,
            0b101 => Self::O32S64,
            0b110 => Self::O16x32S32x64,
            0b111 => Self::O16x32S32,
            _ => unreachable!(),
        }
    }

    pub const fn get_obj_width(&self) -> u8 {
        match self {
            Self::O8S16 | Self::O8S32 | Self::O8S64 => 8,
            Self::O16S32 | Self::O16S64 | Self::O16x32S32 | Self::O16x32S32x64 => 16,
            Self::O32S64 => 32,
        }
    }

    pub const fn get_obj_height(&self) -> u8 {
        match self {
            Self::O8S16 | Self::O8S32 | Self::O8S64 => 8,
            Self::O16S32 | Self::O16S64 => 16,
            Self::O32S64 | Self::O16x32S32 | Self::O16x32S32x64 => 32,
        }
    }

    pub const fn get_sprite_width(&self) -> u8 {
        match self {
            Self::O8S16 => 16,
            Self::O8S32 | Self::O16S32 | Self::O16x32S32 | Self::O16x32S32x64 => 32,
            Self::O8S64 | Self::O16S64 | Self::O32S64 => 64,
        }
    }

    pub const fn get_sprite_height(&self) -> u8 {
        match self {
            Self::O8S16 => 16,
            Self::O8S32 | Self::O16S32 | Self::O16x32S32 => 32,
            Self::O8S64 | Self::O16S64 | Self::O32S64 | Self::O16x32S32x64 => 64,
        }
    }
}
