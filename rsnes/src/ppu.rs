#[derive(Debug, Clone)]
pub struct Ppu {
    /// A value between 0 and 15 with 15 being maximum brightness
    brightness: u8,
}

impl Ppu {
    pub fn new() -> Self {
        Self { brightness: 0x0f }
    }

    /// Write to a PPU register (memory map 0x2100..=0x2133)
    pub fn write_register(&mut self, id: u8, val: u8) {
        match id {
            0x00 => {
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
            0x34.. => unreachable!(),
            _ => todo!(),
        }
    }
}
