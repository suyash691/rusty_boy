const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1], [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1], [0, 1, 1, 1, 1, 1, 1, 0],
];

pub struct PulseChannel {
    pub has_sweep: bool, pub enabled: bool,
    duty: u8, pub length_counter: u8, pub length_enabled: bool,
    volume: u8, volume_initial: u8, envelope_dir: bool, envelope_period: u8, envelope_counter: u8,
    frequency: u16, timer: u16, duty_pos: u8,
    sweep_period: u8, sweep_dir: bool, sweep_shift: u8,
    sweep_counter: u8, sweep_enabled: bool, shadow_freq: u16,
    sweep_negate_used: bool,
}

impl PulseChannel {
    pub fn new(has_sweep: bool) -> Self {
        PulseChannel {
            has_sweep, enabled: false, duty: 0, length_counter: 0, length_enabled: false,
            volume: 0, volume_initial: 0, envelope_dir: false, envelope_period: 0, envelope_counter: 0,
            frequency: 0, timer: 0, duty_pos: 0,
            sweep_period: 0, sweep_dir: false, sweep_shift: 0,
            sweep_counter: 0, sweep_enabled: false, shadow_freq: 0,
            sweep_negate_used: false,
        }
    }
    pub fn tick(&mut self) {
        if self.timer == 0 { self.timer = (2048 - self.frequency) * 4; self.duty_pos = (self.duty_pos + 1) & 7; }
        self.timer = self.timer.saturating_sub(1);
    }
    pub fn output(&self) -> u8 { if !self.enabled { 0 } else { DUTY_TABLE[self.duty as usize][self.duty_pos as usize] * self.volume } }
    pub fn is_active(&self) -> bool { self.enabled }
    pub fn clock_length(&mut self) {
        if self.length_enabled && self.length_counter > 0 { self.length_counter -= 1; if self.length_counter == 0 { self.enabled = false; } }
    }
    pub fn clock_envelope(&mut self) {
        if self.envelope_period == 0 { return; }
        self.envelope_counter = self.envelope_counter.saturating_sub(1);
        if self.envelope_counter == 0 {
            self.envelope_counter = if self.envelope_period > 0 { self.envelope_period } else { 8 };
            if self.envelope_dir && self.volume < 15 { self.volume += 1; }
            else if !self.envelope_dir && self.volume > 0 { self.volume -= 1; }
        }
    }
    pub fn clock_sweep(&mut self) {
        if !self.has_sweep || !self.sweep_enabled { return; }
        self.sweep_counter = self.sweep_counter.saturating_sub(1);
        if self.sweep_counter == 0 {
            self.sweep_counter = if self.sweep_period > 0 { self.sweep_period } else { 8 };
            if self.sweep_period > 0 {
                let nf = self.calc_sweep();
                if nf <= 2047 && self.sweep_shift > 0 { self.frequency = nf; self.shadow_freq = nf; if self.calc_sweep() > 2047 { self.enabled = false; } }
                else if nf > 2047 { self.enabled = false; }
            }
        }
    }
    fn calc_sweep(&mut self) -> u16 {
        let d = self.shadow_freq >> self.sweep_shift;
        if self.sweep_dir { self.sweep_negate_used = true; self.shadow_freq.wrapping_sub(d) } else { self.shadow_freq + d }
    }
    pub fn read_reg(&self, reg: u16) -> u8 {
        match reg {
            0 => if self.has_sweep { 0x80 | (self.sweep_period << 4) | (if self.sweep_dir { 8 } else { 0 }) | self.sweep_shift } else { 0xFF },
            1 => (self.duty << 6) | 0x3F, 2 => (self.volume_initial << 4) | (if self.envelope_dir { 8 } else { 0 }) | self.envelope_period,
            3 => 0xFF, 4 => (if self.length_enabled { 0x40 } else { 0 }) | 0xBF, _ => 0xFF,
        }
    }
    pub fn write_reg(&mut self, reg: u16, value: u8) {
        match reg {
            0 => {
                let old_dir = self.sweep_dir;
                self.sweep_period = (value >> 4) & 7;
                self.sweep_dir = value & 8 != 0;
                self.sweep_shift = value & 7;
                // Quirk: switching from negate to positive after using negate disables channel
                if old_dir && !self.sweep_dir && self.sweep_negate_used { self.enabled = false; }
            }
            1 => { self.duty = (value >> 6) & 3; self.length_counter = 64 - (value & 0x3F); }
            2 => { self.volume_initial = value >> 4; self.envelope_dir = value & 8 != 0; self.envelope_period = value & 7; if self.volume_initial == 0 && !self.envelope_dir { self.enabled = false; } }
            3 => self.frequency = (self.frequency & 0x700) | value as u16,
            4 => {
                self.frequency = (self.frequency & 0xFF) | ((value as u16 & 7) << 8);
                self.length_enabled = value & 0x40 != 0;
                if value & 0x80 != 0 { self.trigger(); }
            }
            _ => {}
        }
    }
    /// Write only the length bits (for APU-off DMG behavior)
    pub fn write_length_only(&mut self, value: u8) { self.length_counter = 64 - (value & 0x3F); }
    fn trigger(&mut self) {
        self.enabled = true; if self.length_counter == 0 { self.length_counter = 64; }
        // Timer: low 2 bits are NOT modified on trigger
        let new_timer = (2048 - self.frequency) * 4;
        self.timer = (new_timer & !3) | (self.timer & 3);
        self.volume = self.volume_initial; self.envelope_counter = if self.envelope_period > 0 { self.envelope_period } else { 8 };
        self.shadow_freq = self.frequency; self.sweep_counter = if self.sweep_period > 0 { self.sweep_period } else { 8 };
        self.sweep_enabled = self.sweep_period > 0 || self.sweep_shift > 0;
        self.sweep_negate_used = false;
        if self.sweep_shift > 0 && self.calc_sweep() > 2047 { self.enabled = false; }
        if self.volume_initial == 0 && !self.envelope_dir { self.enabled = false; }
    }
}
