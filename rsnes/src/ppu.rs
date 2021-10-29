use crate::oam::{CgRam, Oam};
use core::mem::replace;

pub const VRAM_SIZE: usize = 0x8000;
pub const SCREEN_WIDTH: u32 = 340;
pub const MAX_SCREEN_HEIGHT: u32 = 239;

#[derive(Debug, Clone, Copy)]
pub enum BgModeNum {
    Mode0,
    Mode1,
    Mode2,
    Mode3,
    Mode4,
    Mode5,
    Mode6,
    Mode7,
}

#[derive(Debug, Clone, Copy)]
pub struct BgMode {
    num: BgModeNum,
    // only relevant to mode 1
    bg3_priority: bool,
    // only relevant to mode 7
    extbg: bool,
}

impl BgMode {
    pub fn new() -> Self {
        Self {
            num: BgModeNum::Mode0,
            bg3_priority: false,
            extbg: false,
        }
    }

    pub fn set_bits(&mut self, bits: u8) {
        self.num = match bits & 7 {
            0 => BgModeNum::Mode0,
            1 => BgModeNum::Mode1,
            2 => BgModeNum::Mode2,
            3 => BgModeNum::Mode3,
            4 => BgModeNum::Mode4,
            5 => BgModeNum::Mode5,
            6 => BgModeNum::Mode6,
            7 => BgModeNum::Mode7,
            _ => unreachable!(),
        };
        self.bg3_priority = bits & 8 > 0;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
}

#[derive(Debug, Clone, Copy)]
pub struct Background {
    tilemap_addr: u8,
    base_addr: u8,
    x_mirror: bool,
    y_mirror: bool,
    // otherwise it is 8x8
    is_16x16_tiles: bool,
    scroll_prev: u8,
    scroll_prev_h: u8,
    scroll: [u16; 2],
    layer: Layer,
}

impl Background {
    pub const fn new() -> Self {
        Self {
            tilemap_addr: 0,
            base_addr: 0,
            x_mirror: false,
            y_mirror: false,
            is_16x16_tiles: false,
            scroll_prev: 0,
            scroll_prev_h: 0,
            scroll: [0; 2],
            layer: Layer::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Layer {
    mask_logic: MaskLogic,
    window1: bool,
    window2: bool,
    window1_inversion: bool,
    window2_inversion: bool,
    color_math: bool,
    // not on color_layer
    main_screen: bool,
    // not on color_layer
    sub_screen: bool,
    // not on color_layer
    main_screen_masked: bool,
    // not on color_layer
    sub_screen_masked: bool,
}

impl Layer {
    pub const fn new() -> Self {
        Self {
            mask_logic: MaskLogic::Or,
            window1: false,
            window2: false,
            window1_inversion: false,
            window2_inversion: false,
            color_math: false,
            main_screen: false,
            sub_screen: false,
            main_screen_masked: false,
            sub_screen_masked: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskLogic {
    Or,
    And,
    Xor,
    XNor,
}

impl MaskLogic {
    pub fn from_byte(val: u8) -> Self {
        match val & 3 {
            0 => Self::Or,
            1 => Self::And,
            2 => Self::Xor,
            3 => Self::XNor,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mode7Settings {
    x_mirror: bool,
    y_mirror: bool,
    fill_zeros: Option<bool>,
    prev: u8,
    offset: [i16; 2],
}

impl Mode7Settings {
    pub fn write_offset(&mut self, is_vertical: bool, val: u8) {
        let i = is_vertical as usize;
        self.offset[i] = i16::from(replace(&mut self.prev, val)) | (i16::from(val) << 8);
        if self.offset[i] & 0x1000 > 0 {
            self.offset[i] |= -0x2000
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ppu<FB: crate::backend::FrameBuffer> {
    pub(crate) oam: Oam,
    pub frame_buffer: FB,
    cgram: CgRam,
    vram: [u16; VRAM_SIZE],
    vram_addr_unmapped: u16,
    /// A value between 0 and 15 with 15 being maximum brightness
    brightness: u8,
    obj_size: ObjectSize,
    remap_mode: RemapMode,
    vram_increment_amount: u8,
    increment_first: bool,
    pub(crate) overscan: bool,
    mosaic_bgs: u8,
    mosaic_size: u8,
    bgs: [Background; 4],
    obj_layer: Layer,
    color_layer: Layer,
    mode7_settings: Mode7Settings,
    direct_color_mode: bool,
    add_subscreen: bool,
    color_behaviour: u8,
    subtract_color: bool,
    half_color: bool,
    fixed_color: Color,
    bg_mode: BgMode,
    window_positions: [[u8; 2]; 2],
}

impl<FB: crate::backend::FrameBuffer> Ppu<FB> {
    pub fn new(frame_buffer: FB) -> Self {
        Self {
            oam: Oam::new(),
            frame_buffer,
            cgram: CgRam::new(),
            vram: [0; VRAM_SIZE],
            vram_addr_unmapped: 0,
            brightness: 0x0f,
            obj_size: ObjectSize::O8S16,
            remap_mode: RemapMode::NoRemap,
            vram_increment_amount: 1,
            increment_first: true,
            overscan: false,
            mosaic_bgs: 0,
            mosaic_size: 0,
            bgs: [Background::new(); 4],
            obj_layer: Layer::new(),
            color_layer: Layer::new(),
            mode7_settings: Mode7Settings {
                x_mirror: false,
                y_mirror: false,
                fill_zeros: None,
                prev: 0,
                offset: [0; 2],
            },
            direct_color_mode: false,
            add_subscreen: false,
            color_behaviour: 0,
            subtract_color: false,
            half_color: false,
            fixed_color: Color::BLACK,
            bg_mode: BgMode::new(),
            window_positions: [[0; 2]; 2],
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
            0x02 => {
                // OAMADDL
                self.oam.set_addr_low(val)
            }
            0x03 => {
                // OAMADDH
                self.oam.set_addr_high(val)
            }
            0x04 => {
                // OAMDATA
                self.oam.write(val)
            }
            0x05 => {
                // BGMODE
                self.bg_mode.set_bits(val);
                for (i, bg) in self.bgs.iter_mut().enumerate() {
                    bg.is_16x16_tiles = val & (1 << (i | 4)) > 0;
                }
            }
            0x06 => {
                // MOSAIC
                self.mosaic_size = val >> 4;
                self.mosaic_bgs = val & 0xf;
            }
            0x07..=0x0a => {
                // BGnSC
                let bg = &mut self.bgs[usize::from((id + 1) & 3)];
                bg.tilemap_addr = (val & 0x7f) >> 2;
                bg.y_mirror = val & 2 > 0;
                bg.x_mirror = val & 1 > 0;
            }
            0x0b..=0x0c => {
                // BGnmNBA
                let val = val & 0x77;
                let id = usize::from(!id & 2);
                self.bgs[id].base_addr = val >> 4;
                self.bgs[id | 1].base_addr = val & 7;
            }
            0x0d..=0x14 => {
                // M7xOFS and BGnxOFS
                if (0x0d..=0x14).contains(&id) {
                    self.mode7_settings.write_offset(id & 1 == 0, val)
                }
                let bg = &mut self.bgs[usize::from(((id - 5) >> 1) & 3)];
                let old = replace(&mut bg.scroll_prev, val);
                bg.scroll[usize::from(!id & 1)] = u16::from(val)
                    | u16::from(if id & 1 > 0 {
                        (old & 0xf8) | replace(&mut bg.scroll_prev_h, val) & 7
                    } else {
                        old
                    });
            }
            0x15 => {
                // VMAIN - Video Port Control
                self.increment_first = val & 0x80 == 0;
                self.vram_increment_amount = match val & 0b11 {
                    0 => 1,
                    1 => 32,
                    _ => 128,
                };
                self.remap_mode = RemapMode::from_bits(val >> 2);
            }
            0x16 => {
                // VMADDL
                self.vram_addr_unmapped = (self.vram_addr_unmapped & 0xff00) | u16::from(val);
            }
            0x17 => {
                // VMADDH
                self.vram_addr_unmapped = (self.vram_addr_unmapped & 0xff) | (u16::from(val) << 8);
            }
            0x18 => {
                // VMDATAL
                let word = self.get_vram_word_mut(self.vram_addr_unmapped);
                *word = (*word & 0xff00) | u16::from(val);
                if self.increment_first {
                    self.vram_addr_unmapped = self
                        .vram_addr_unmapped
                        .wrapping_add(self.vram_increment_amount.into());
                }
            }
            0x19 => {
                // VMDATAH
                let word = self.get_vram_word_mut(self.vram_addr_unmapped);
                *word = (*word & 0xff) | (u16::from(val) << 8);
                if !self.increment_first {
                    self.vram_addr_unmapped = self
                        .vram_addr_unmapped
                        .wrapping_add(self.vram_increment_amount.into());
                }
            }
            0x1a => {
                // M7SEL
                self.mode7_settings.x_mirror = val & 1 > 0;
                self.mode7_settings.y_mirror = val & 2 > 0;
                self.mode7_settings.fill_zeros = Some(val & 0x40 > 0).filter(|_| val & 0x80 > 0);
            }
            0x21 => {
                // CGADD
                self.cgram.set_addr(val)
            }
            0x22 => {
                // CGADD
                self.cgram.write(val)
            }
            0x23..=0x24 => {
                // WnmSEL
                let mut val = val;
                for i in 0..2 {
                    let bg = &mut self.bgs[usize::from(i + (!id & 2))];
                    bg.layer.window1_inversion = val & 1 > 0;
                    bg.layer.window1 = val & 2 > 0;
                    bg.layer.window2_inversion = val & 4 > 0;
                    bg.layer.window2 = val & 8 > 0;
                    val >>= 4;
                }
            }
            0x25 => {
                // WOBJSEL
                let mut val = val;
                for layer in [&mut self.obj_layer, &mut self.color_layer] {
                    layer.window1_inversion = val & 1 > 0;
                    layer.window1 = val & 2 > 0;
                    layer.window2_inversion = val & 4 > 0;
                    layer.window2 = val & 8 > 0;
                    val >>= 4;
                }
            }
            0x26..=0x29 => {
                // WH0-3
                self.window_positions[usize::from((!id & 2) >> 1)][usize::from(id & 1)] = val
            }
            0x2a => {
                // WBGLOG
                for i in 0..4 {
                    self.bgs[i].layer.mask_logic = MaskLogic::from_byte(val >> (i << 1));
                }
            }
            0x2b => {
                // WOBJLOG
                self.obj_layer.mask_logic = MaskLogic::from_byte(val);
                self.color_layer.mask_logic = MaskLogic::from_byte(val >> 2);
            }
            0x2c..=0x2f => {
                // TM/TS/TMW/TSW
                let f: fn(&mut Layer, val: bool) = match id {
                    0x2c => |layer, val| layer.main_screen = val,
                    0x2d => |layer, val| layer.sub_screen = val,
                    0x2e => |layer, val| layer.main_screen_masked = val,
                    0x2f => |layer, val| layer.sub_screen_masked = val,
                    _ => unreachable!(),
                };
                for (i, bg) in self.bgs.iter_mut().enumerate() {
                    f(&mut bg.layer, val & (1 << i) > 0)
                }
                f(&mut self.obj_layer, val & 0x10 > 0)
            }
            0x30 => {
                // CGWSEL
                self.direct_color_mode = val & 1 > 0;
                self.add_subscreen = val & 2 > 0;
                self.color_behaviour = val >> 4;
            }
            0x31 => {
                // CGADSUB
                self.bgs[0].layer.color_math = val & 1 > 0;
                self.bgs[1].layer.color_math = val & 2 > 0;
                self.bgs[2].layer.color_math = val & 4 > 0;
                self.bgs[3].layer.color_math = val & 8 > 0;
                self.obj_layer.color_math = val & 0x10 > 0;
                self.color_layer.color_math = val & 0x20 > 0;
                self.half_color = val & 0x40 > 0;
                self.subtract_color = val & 0x80 > 0;
            }
            0x32 => {
                // COLDATA
                if val & 0x20 > 0 {
                    self.fixed_color.r = val
                }
                if val & 0x40 > 0 {
                    self.fixed_color.g = val
                }
                if val & 0x80 > 0 {
                    self.fixed_color.b = val
                }
            }
            0x33 => {
                // SETINI
                if val & 1 > 0 {
                    todo!("screen interlace mode")
                }
                if val & 2 > 0 {
                    todo!("object interlace mode")
                }
                self.overscan = val & 4 > 0;
                if val & 8 > 0 {
                    todo!("pseudo-hires mode")
                }
                self.bg_mode.extbg = val & 0x40 > 0;
                if val & 0x80 > 0 {
                    todo!("enable super imposing")
                }
            }
            0x34.. => unreachable!(),
            _ => todo!("write to unknown PPU register 0x21{:02x}", id),
        }
    }

    fn get_vram_word_mut(&mut self, index: u16) -> &mut u16 {
        self.vram
            .get_mut(usize::from(self.remap_mode.remap(index) & 0x7fff))
            .unwrap()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemapMode {
    NoRemap,
    First,
    Second,
    Third,
}

impl RemapMode {
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0b00 => Self::NoRemap,
            0b01 => Self::First,
            0b10 => Self::Second,
            0b11 => Self::Third,
            _ => unreachable!(),
        }
    }

    pub const fn remap(&self, addr: u16) -> u16 {
        match self {
            Self::NoRemap => addr,
            Self::First => (addr & 0xff00) | ((addr & 0x1f) << 3) | ((addr >> 5) & 0b111),
            Self::Second => (addr & 0xfe00) | ((addr & 0x3f) << 3) | ((addr >> 6) & 0b111),
            Self::Third => (addr & 0xfc00) | ((addr & 0x7f) << 3) | ((addr >> 7) & 0b111),
        }
    }
}
