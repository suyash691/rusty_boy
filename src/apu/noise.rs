pub struct NoiseChannel {
    pub enabled: bool,
    pub length_counter: u8, pub length_enabled: bool,
    volume: u8, volume_initial: u8, envelope_dir: bool, envelope_period: u8, envelope_counter: u8,
    shift_clock: u8, width_mode: bool, divisor_code: u8,
    timer: u16, lfsr: u16,
}

impl NoiseChannel {
    pub fn new() -> Self {
        NoiseChannel { enabled: false, length_counter: 0, length_enabled: false, volume: 0, volume_initial: 0, envelope_dir: false, envelope_period: 0, envelope_counter: 0, shift_clock: 0, width_mode: false, divisor_code: 0, timer: 0, lfsr: 0x7FFF }
    }
    pub fn tick(&mut self) {
        if self.timer == 0 {
            self.timer = self.period();
            let xor = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            self.lfsr = (self.lfsr >> 1) | (xor << 14);
            if self.width_mode { self.lfsr = (self.lfsr & !(1 << 6)) | (xor << 6); }
        }
        self.timer = self.timer.saturating_sub(1);
    }
    fn period(&self) -> u16 { let d = match self.divisor_code { 0 => 8, n => n as u16 * 16 }; d << self.shift_clock }
    pub fn output(&self) -> u8 { if !self.enabled { 0 } else if self.lfsr & 1 == 0 { self.volume } else { 0 } }
    pub fn is_active(&self) -> bool { self.enabled }
    pub fn clock_length(&mut self) { if self.length_enabled && self.length_counter > 0 { self.length_counter -= 1; if self.length_counter == 0 { self.enabled = false; } } }
    pub fn clock_envelope(&mut self) {
        if self.envelope_period == 0 { return; }
        self.envelope_counter = self.envelope_counter.saturating_sub(1);
        if self.envelope_counter == 0 { self.envelope_counter = if self.envelope_period > 0 { self.envelope_period } else { 8 }; if self.envelope_dir && self.volume < 15 { self.volume += 1; } else if !self.envelope_dir && self.volume > 0 { self.volume -= 1; } }
    }
    pub fn read_reg(&self, reg: u16) -> u8 {
        match reg { 0 | 1 => 0xFF, 2 => (self.volume_initial << 4) | (if self.envelope_dir { 8 } else { 0 }) | self.envelope_period, 3 => (self.shift_clock << 4) | (if self.width_mode { 8 } else { 0 }) | self.divisor_code, 4 => (if self.length_enabled { 0x40 } else { 0 }) | 0xBF, _ => 0xFF }
    }
    pub fn write_reg(&mut self, reg: u16, value: u8) {
        match reg {
            0 => {} 1 => self.length_counter = 64 - (value & 0x3F),
            2 => { self.volume_initial = value >> 4; self.envelope_dir = value & 8 != 0; self.envelope_period = value & 7; if self.volume_initial == 0 && !self.envelope_dir { self.enabled = false; } }
            3 => { self.shift_clock = value >> 4; self.width_mode = value & 8 != 0; self.divisor_code = value & 7; }
            4 => { self.length_enabled = value & 0x40 != 0; if value & 0x80 != 0 { self.trigger(); } }
            _ => {}
        }
    }
    fn trigger(&mut self) { self.enabled = true; if self.length_counter == 0 { self.length_counter = 64; } self.timer = self.period(); self.lfsr = 0x7FFF; self.volume = self.volume_initial; self.envelope_counter = if self.envelope_period > 0 { self.envelope_period } else { 8 }; if self.volume_initial == 0 && !self.envelope_dir { self.enabled = false; } }
}
