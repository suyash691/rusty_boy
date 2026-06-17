pub struct WaveChannel {
    pub enabled: bool, dac_enabled: bool,
    length_counter: u16, length_enabled: bool,
    volume_shift: u8, frequency: u16, timer: u16, position: u8,
    wave_ram: [u8; 16],
}

impl WaveChannel {
    pub fn new() -> Self {
        WaveChannel { enabled: false, dac_enabled: false, length_counter: 0, length_enabled: false, volume_shift: 0, frequency: 0, timer: 0, position: 0, wave_ram: [0; 16] }
    }
    pub fn tick(&mut self) {
        if self.timer == 0 { self.timer = (2048 - self.frequency) * 2; self.position = (self.position + 1) & 31; }
        self.timer = self.timer.saturating_sub(1);
    }
    pub fn output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled { return 0; }
        let s = if self.position & 1 == 0 { self.wave_ram[self.position as usize / 2] >> 4 } else { self.wave_ram[self.position as usize / 2] & 0xF };
        if self.volume_shift == 0 { 0 } else { s >> (self.volume_shift - 1) }
    }
    pub fn is_active(&self) -> bool { self.enabled }
    pub fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 { self.length_counter -= 1; if self.length_counter == 0 { self.enabled = false; } }
    }
    pub fn power_off(&mut self) { self.enabled = false; self.dac_enabled = false; self.volume_shift = 0; self.frequency = 0; self.length_enabled = false; }
    pub fn get_length(&self) -> u16 { self.length_counter }
    pub fn set_length(&mut self, v: u16) { self.length_counter = v; }
    pub fn length_enabled_flag(&self) -> bool { self.length_enabled }
    pub fn read_reg(&self, reg: u16) -> u8 {
        match reg { 0 => (if self.dac_enabled { 0x80 } else { 0 }) | 0x7F, 1 => 0xFF, 2 => (self.volume_shift << 5) | 0x9F, 3 => 0xFF, 4 => (if self.length_enabled { 0x40 } else { 0 }) | 0xBF, _ => 0xFF }
    }
    pub fn write_reg(&mut self, reg: u16, value: u8) {
        match reg {
            0 => { self.dac_enabled = value & 0x80 != 0; if !self.dac_enabled { self.enabled = false; } }
            1 => self.length_counter = 256 - value as u16,
            2 => self.volume_shift = (value >> 5) & 3,
            3 => self.frequency = (self.frequency & 0x700) | value as u16,
            4 => { self.frequency = (self.frequency & 0xFF) | ((value as u16 & 7) << 8); self.length_enabled = value & 0x40 != 0; if value & 0x80 != 0 { self.trigger(); } }
            _ => {}
        }
    }
    fn trigger(&mut self) { self.enabled = self.dac_enabled; if self.length_counter == 0 { self.length_counter = 256; } self.timer = (2048 - self.frequency) * 2; self.position = 0; }
    pub fn read_wave_ram(&self, offset: u16) -> u8 { self.wave_ram[offset as usize] }
    pub fn write_wave_ram(&mut self, offset: u16, value: u8) { self.wave_ram[offset as usize] = value; }
}
