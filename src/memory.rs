pub struct Memory {
    ram: [u8; 0x10000], // 64KB of memory
}

impl Memory {
    pub fn new() -> Self {
        Memory {
            ram: [0; 0x10000],
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        self.ram[address as usize]
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        self.ram[address as usize] = value;
    }
}