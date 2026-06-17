mod pulse;
mod wave;
mod noise;

use pulse::PulseChannel;
use wave::WaveChannel;
use noise::NoiseChannel;

pub struct APU {
    enabled: bool,
    ch1: PulseChannel,
    ch2: PulseChannel,
    ch3: WaveChannel,
    ch4: NoiseChannel,
    // Frame sequencer
    pub frame_seq_step: u8,
    prev_div_apu_bit: bool,
    // Mixing
    nr50: u8, // Volume/VIN
    nr51: u8, // Panning
    // Output buffer
    pub sample_buffer: Vec<f32>,
    sample_rate_counter: u32,
}

const SAMPLE_EVERY: u32 = 95; // ~44100 Hz at 4.19 MHz (4194304/44100 ≈ 95)

impl APU {
    pub fn new() -> Self {
        APU {
            enabled: false,
            ch1: PulseChannel::new(true),
            ch2: PulseChannel::new(false),
            ch3: WaveChannel::new(),
            ch4: NoiseChannel::new(),
            frame_seq_step: 0,
            prev_div_apu_bit: false,
            nr50: 0,
            nr51: 0,
            sample_buffer: Vec::with_capacity(1024),
            sample_rate_counter: 0,
        }
    }

    pub fn update_with_div(&mut self, cycles: u32, div_bit: bool) {
        if !self.enabled { return; }

        for _ in 0..cycles {
            self.ch1.tick();
            self.ch2.tick();
            self.ch3.tick();
            self.ch4.tick();

            // Frame sequencer: clocked by falling edge of DIV bit 12
            if self.prev_div_apu_bit && !div_bit {
                self.clock_frame_sequencer();
            }
            self.prev_div_apu_bit = div_bit;

            // Sample output
            self.sample_rate_counter += 1;
            if self.sample_rate_counter >= SAMPLE_EVERY {
                self.sample_rate_counter = 0;
                self.output_sample();
            }
        }
    }

    fn clock_frame_sequencer(&mut self) {
        match self.frame_seq_step {
            0 => { self.ch1.clock_length(); self.ch2.clock_length(); self.ch3.clock_length(); self.ch4.clock_length(); }
            2 => { self.ch1.clock_length(); self.ch2.clock_length(); self.ch3.clock_length(); self.ch4.clock_length(); self.ch1.clock_sweep(); }
            4 => { self.ch1.clock_length(); self.ch2.clock_length(); self.ch3.clock_length(); self.ch4.clock_length(); }
            6 => { self.ch1.clock_length(); self.ch2.clock_length(); self.ch3.clock_length(); self.ch4.clock_length(); self.ch1.clock_sweep(); }
            7 => { self.ch1.clock_envelope(); self.ch2.clock_envelope(); self.ch4.clock_envelope(); }
            _ => {}
        }
        self.frame_seq_step = (self.frame_seq_step + 1) & 7;
    }

    fn output_sample(&mut self) {
        let ch1 = self.ch1.output() as f32;
        let ch2 = self.ch2.output() as f32;
        let ch3 = self.ch3.output() as f32;
        let ch4 = self.ch4.output() as f32;

        let left_vol = ((self.nr50 >> 4) & 7) as f32 + 1.0;
        let right_vol = (self.nr50 & 7) as f32 + 1.0;

        let mut left: f32 = 0.0;
        let mut right: f32 = 0.0;

        if self.nr51 & 0x10 != 0 { left += ch1; }
        if self.nr51 & 0x20 != 0 { left += ch2; }
        if self.nr51 & 0x40 != 0 { left += ch3; }
        if self.nr51 & 0x80 != 0 { left += ch4; }
        if self.nr51 & 0x01 != 0 { right += ch1; }
        if self.nr51 & 0x02 != 0 { right += ch2; }
        if self.nr51 & 0x04 != 0 { right += ch3; }
        if self.nr51 & 0x08 != 0 { right += ch4; }

        left = (left * left_vol) / (4.0 * 8.0);
        right = (right * right_vol) / (4.0 * 8.0);

        self.sample_buffer.push(left);
        self.sample_buffer.push(right);
    }

    /// Drain all generated samples (interleaved stereo L/R f32) for the audio sink,
    /// emptying the internal buffer. Call once per frame so it never grows unbounded.
    pub fn take_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.sample_buffer)
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0xFF10..=0xFF14 => self.ch1.read_reg(addr - 0xFF10),
            0xFF15..=0xFF19 => self.ch2.read_reg(addr - 0xFF15),
            0xFF1A..=0xFF1E => self.ch3.read_reg(addr - 0xFF1A),
            0xFF1F..=0xFF23 => self.ch4.read_reg(addr - 0xFF1F),
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => {
                let mut v = if self.enabled { 0x80 } else { 0 };
                if self.ch1.is_active() { v |= 0x01; }
                if self.ch2.is_active() { v |= 0x02; }
                if self.ch3.is_active() { v |= 0x04; }
                if self.ch4.is_active() { v |= 0x08; }
                v | 0x70
            }
            0xFF30..=0xFF3F => self.ch3.read_wave_ram(addr - 0xFF30),
            _ => 0xFF,
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        // When APU is off, only NR52, wave RAM, and length counters (NRx1) are writable on DMG
        if !self.enabled && addr != 0xFF26 && !(0xFF30..=0xFF3F).contains(&addr) {
            // On DMG, NRx1 length-only writes: only low 6 bits (length) writable, not duty
            match addr {
                0xFF11 => { self.ch1.write_length_only(value & 0x3F); return; }
                0xFF16 => { self.ch2.write_length_only(value & 0x3F); return; }
                0xFF1B => { self.ch3.write_reg(1, value); return; }
                0xFF20 => { self.ch4.write_reg(1, value & 0x3F); return; }
                _ => return,
            }
        }
        match addr {
            0xFF10..=0xFF14 => {
                let old_len_en = self.ch1.length_enabled;
                self.ch1.write_reg(addr - 0xFF10, value);
                if addr == 0xFF14 && value & 0x40 != 0 && !old_len_en && self.ch1.length_counter > 0 && self.should_extra_clock() {
                    self.ch1.length_counter -= 1;
                    if self.ch1.length_counter == 0 && value & 0x80 == 0 { self.ch1.enabled = false; }
                }
            }
            0xFF15..=0xFF19 => {
                let old_len_en = self.ch2.length_enabled;
                self.ch2.write_reg(addr - 0xFF15, value);
                if addr == 0xFF19 && value & 0x40 != 0 && !old_len_en && self.ch2.length_counter > 0 && self.should_extra_clock() {
                    self.ch2.length_counter -= 1;
                    if self.ch2.length_counter == 0 && value & 0x80 == 0 { self.ch2.enabled = false; }
                }
            }
            0xFF1A..=0xFF1E => {
                let old_len_en = self.ch3.length_enabled_flag();
                self.ch3.write_reg(addr - 0xFF1A, value);
                if addr == 0xFF1E { self.extra_length_clock_ch3(value, old_len_en); }
            }
            0xFF1F..=0xFF23 => {
                let old_len_en = self.ch4.length_enabled;
                self.ch4.write_reg(addr - 0xFF1F, value);
                if addr == 0xFF23 { self.extra_length_clock_ch4(value, old_len_en); }
            }
            0xFF24 => self.nr50 = value,
            0xFF25 => self.nr51 = value,
            0xFF26 => {
                let was_enabled = self.enabled;
                self.enabled = value & 0x80 != 0;
                if was_enabled && !self.enabled {
                    self.power_off();
                } else if !was_enabled && self.enabled {
                    self.frame_seq_step = 0; // Reset sequencer on power on
                }
            }
            0xFF30..=0xFF3F => self.ch3.write_wave_ram(addr - 0xFF30, value),
            _ => {}
        }
    }


    // Extra length clocking: when enabling length on a non-length frame step
    fn should_extra_clock(&self) -> bool { self.frame_seq_step & 1 != 0 }

    fn extra_length_clock_ch3(&mut self, value: u8, old_len_en: bool) {
        let lc = self.ch3.get_length();
        if value & 0x40 != 0 && !old_len_en && lc > 0 && self.should_extra_clock() {
            self.ch3.set_length(lc - 1);
            if lc - 1 == 0 && value & 0x80 == 0 { self.ch3.enabled = false; }
        }
    }
    fn extra_length_clock_ch4(&mut self, value: u8, old_len_en: bool) {
        if value & 0x40 != 0 && !old_len_en && self.ch4.length_counter > 0 && self.should_extra_clock() {
            self.ch4.length_counter -= 1;
            if self.ch4.length_counter == 0 && value & 0x80 == 0 { self.ch4.enabled = false; }
        }
    }
    fn power_off(&mut self) {
        // DMG: length counters are preserved across power off
        let l1 = self.ch1.length_counter;
        let l2 = self.ch2.length_counter;
        let l3 = self.ch3.get_length();
        let l4 = self.ch4.length_counter;
        self.ch1 = PulseChannel::new(true);
        self.ch2 = PulseChannel::new(false);
        self.ch3.power_off();
        self.ch4 = NoiseChannel::new();
        self.ch1.length_counter = l1;
        self.ch2.length_counter = l2;
        self.ch3.set_length(l3);
        self.ch4.length_counter = l4;
        self.nr50 = 0;
        self.nr51 = 0;
        self.frame_seq_step = 0;
    }
}
