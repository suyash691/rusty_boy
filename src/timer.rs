pub struct Timer {
    div: u16,  // 16-bit internal DIV counter
    tima: u8,  // Timer Counter
    tma: u8,   // Timer Modulo
    tac: u8,   // Timer Control
    last_bit: bool, // Last bit state for edge detection
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            last_bit: false,
        }
    }

    pub fn update(&mut self, cycles: u32) {
        for _ in 0..cycles {
            self.tick();
        }
    }

    fn tick(&mut self) {
        // Increment internal DIV counter
        self.div = self.div.wrapping_add(1);

        // Check if TIMA should be incremented
        let current_bit = self.should_increment();
        if !self.last_bit && current_bit {
            self.increment_tima();
        }
        self.last_bit = current_bit;
    }

    fn should_increment(&self) -> bool {
        if self.tac & 0x04 == 0 {
            return false;
        }

        let bit_to_check = match self.tac & 0x03 {
            0 => 9,  // CPU Clock / 1024 (check bit 9 of DIV)
            1 => 3,  // CPU Clock / 16 (check bit 3 of DIV)
            2 => 5,  // CPU Clock / 64 (check bit 5 of DIV)
            3 => 7,  // CPU Clock / 256 (check bit 7 of DIV)
            _ => unreachable!(),
        };

        (self.div & (1 << bit_to_check)) != 0
    }

    fn increment_tima(&mut self) {
        if self.tima == 0xFF {
            // TIMA overflow
            self.tima = self.tma;
            // TODO: Request timer interrupt
        } else {
            self.tima = self.tima.wrapping_add(1);
        }
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac,
            _ => panic!("Invalid timer register address"),
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF04 => self.div = 0,
            0xFF05 => self.tima = value,
            0xFF06 => self.tma = value,
            0xFF07 => self.tac = value & 0x07,
            _ => panic!("Invalid timer register address"),
        }
    }
}