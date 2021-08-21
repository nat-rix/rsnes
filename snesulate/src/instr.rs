use crate::cpu::Status;
use crate::device::Device;

#[rustfmt::skip]
static CYCLES: [u8; 256] = [
    /* ^0 ^1 ^2 ^3 ^4 ^5 ^6 ^7 | ^8 ^9 ^a ^b ^c ^d ^e ^f */
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 0^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 1^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 2^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 3^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 4^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 5^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 6^
       0, 0, 0, 0, 0, 0, 0, 0,   2, 0, 0, 0, 0, 0, 0, 0,  // 7^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // 8^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 4, 0, 0, 0,  // 9^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // a^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // b^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // c^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // d^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // e^
       0, 0, 0, 0, 0, 0, 0, 0,   0, 0, 0, 0, 0, 0, 0, 0,  // f^
];

impl Device {
    pub fn dispatch_instruction_with(&mut self, op: u8) {
        println!("exec '{:02x}'", op);
        let mut cycles = CYCLES[op as usize];
        match op {
            0x78 => {
                // SEI
                self.cpu.regs.status |= Status::IRQ_DISABLE
            }
            0x9c => {
                // STZ - absolute addressing
                if self.cpu.regs.status.has(Status::ACCUMULATION) {
                    let addr = self.load::<u16>();
                    self.write(self.cpu.get_data_addr(addr), 0u8);
                } else {
                    let addr = self.load::<u16>();
                    self.write(self.cpu.get_data_addr(addr), 0u16);
                    cycles += 1;
                }
            }
            _ => todo!(),
        }
    }

    pub fn dispatch_instruction(&mut self) {
        let op = self.load::<u8>();
        self.dispatch_instruction_with(op)
    }
}
