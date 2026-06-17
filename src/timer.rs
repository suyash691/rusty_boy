pub struct Timer {
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    last_bit: bool,
    pub interrupt_requested: bool,
    pub(crate) overflow_countdown: u8,
    overflow_cancelled: bool,
    pub(crate) reload_happened: bool,
    pub pre_write_bit: bool,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            div: 0, tima: 0, tma: 0, tac: 0, last_bit: false,
            interrupt_requested: false, overflow_countdown: 0,
            overflow_cancelled: false, reload_happened: false,
            pre_write_bit: false,
        }
    }

    pub fn update(&mut self, cycles: u32) {
        self.reload_happened = false;
        for _ in 0..cycles { self.tick(); }
    }

    pub fn get_timer_bit_pub(&self) -> bool { self.get_timer_bit() }

    pub fn div_counter(&self) -> u16 { self.div }
    pub fn set_div(&mut self, val: u16) { self.div = val; self.last_bit = self.get_timer_bit(); }

    fn tick(&mut self) {
        // A previously-armed overflow reloads TMA into TIMA and raises the
        // interrupt exactly 4 T-cycles after the increment that wrapped TIMA.
        // `reload_happened` marks the single T-cycle on which the reload lands
        // (cycle B) so register writes on that cycle can be handled correctly.
        self.reload_happened = false;
        if self.overflow_countdown > 0 {
            self.overflow_countdown -= 1;
            if self.overflow_countdown == 0 {
                if !self.overflow_cancelled {
                    self.tima = self.tma;
                    self.interrupt_requested = true;
                    self.reload_happened = true;
                }
                self.overflow_cancelled = false;
            }
        }

        self.div = self.div.wrapping_add(1);

        let current_bit = self.get_timer_bit();
        if self.last_bit && !current_bit {
            self.increment_tima();
        }
        self.last_bit = current_bit;
    }

    fn get_timer_bit(&self) -> bool {
        if self.tac & 0x04 == 0 { return false; }
        let bit = match self.tac & 0x03 {
            0 => 9, 1 => 3, 2 => 5, 3 => 7, _ => unreachable!(),
        };
        (self.div & (1 << bit)) != 0
    }

    fn increment_tima(&mut self) {
        let (result, overflow) = self.tima.overflowing_add(1);
        if overflow {
            self.tima = 0;
            self.overflow_countdown = 4;
            self.overflow_cancelled = false;
        } else {
            self.tima = result;
        }
    }

    /// Increment TIMA with immediate overflow (no 4-cycle delay).
    /// Used when overflow is caused by a register write glitch.
    fn increment_tima_immediate(&mut self) {
        let (result, overflow) = self.tima.overflowing_add(1);
        if overflow {
            self.tima = self.tma;
            self.interrupt_requested = true;
        } else {
            self.tima = result;
        }
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8,
            _ => 0xFF,
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF04 => {
                let bit_high = self.get_timer_bit();
                log::trace!("DIV WRITE: div={} bit_high={} tima={} tac={:02X}", self.div, bit_high, self.tima, self.tac);
                self.div = 0;
                self.last_bit = false;
                if bit_high { self.increment_tima(); log::trace!("  -> TIMA incremented to {}", self.tima); }
            }
            0xFF05 => {
                log::trace!("TIMA WRITE: val={:02X} countdown={} reload_happened={} tima={}", value, self.overflow_countdown, self.reload_happened, self.tima);
                if self.reload_happened {
                    // Cycle B: the reload landed this M-cycle, TMA wins — write ignored.
                } else {
                    // Cycle A (overflow pending): the write cancels the pending reload
                    // and the written value stays. Otherwise a plain write.
                    if self.overflow_countdown > 0 { self.overflow_cancelled = true; }
                    self.tima = value;
                }
            }
            0xFF06 => {
                self.tma = value;
                if self.reload_happened { self.tima = value; self.reload_happened = false; }
            }
            0xFF07 => {
                let old_tac = self.tac;
                let old_bit = if old_tac & 0x04 != 0 {
                    // Timer was enabled: use pre-write bit for accurate edge detection
                    self.pre_write_bit
                } else {
                    false
                };
                log::trace!("TAC WRITE: tac {:02X}->{:02X} div={} old_bit={}", self.tac, value & 7, self.div, old_bit);
                self.tac = value & 0x07;
                let new_bit = self.get_timer_bit();
                if old_bit && !new_bit { self.increment_tima_immediate(); log::trace!("  TAC glitch: TIMA++"); }
                self.last_bit = new_bit;
            }
            _ => {}
        }
    }
}
