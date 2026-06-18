pub const VBLANK_INTERRUPT: u8 = 1 << 0;
pub const LCD_STAT_INTERRUPT: u8 = 1 << 1;
pub const TIMER_INTERRUPT: u8 = 1 << 2;
pub const SERIAL_INTERRUPT: u8 = 1 << 3;
pub const JOYPAD_INTERRUPT: u8 = 1 << 4;

pub struct InterruptController {
    pub ie: u8,
    pub if_: u8,
    /// STAT IRQ raised but not yet committed to the CPU-readable `if_`. Hardware reads
    /// $FF0F through a read-latch (GateBoy MATY/MOPO), so a STAT edge becomes CPU-readable
    /// a fixed delay AFTER it is raised (the PPU-edge path), even though it is dispatchable
    /// immediately. `stat_pending` = dispatchable now; `stat_publish_phase` = the
    /// `phase_lcd` at which it becomes readable via `if_`. (gbmicrotest hblank_int_*_if_*.)
    pub stat_pending: bool,
    pub stat_publish_phase: i64,
}

impl InterruptController {
    pub fn new() -> Self {
        InterruptController { ie: 0, if_: 0, stat_pending: false, stat_publish_phase: 0 }
    }

    pub fn pending(&self) -> u8 {
        // Dispatch sees a deferred STAT edge immediately (edge time unchanged); only the
        // CPU-readable `if_` lags by the read-latch delay.
        let if_eff = self.if_ | if self.stat_pending { LCD_STAT_INTERRUPT } else { 0 };
        self.ie & if_eff
    }

    pub fn request(&mut self, interrupt: u8) {
        self.if_ |= interrupt;
    }

    /// Mark a STAT edge raised, readable into `if_` once `phase_lcd >= publish_phase`.
    pub fn request_stat(&mut self, publish_phase: i64) {
        self.stat_pending = true;
        self.stat_publish_phase = publish_phase;
    }

    /// Commit a pending STAT edge into the CPU-readable `if_` once its publish phase passes.
    pub fn commit_stat_if(&mut self, phase_lcd: i64) {
        if self.stat_pending && phase_lcd >= self.stat_publish_phase {
            self.if_ |= LCD_STAT_INTERRUPT;
            self.stat_pending = false;
        }
    }

    pub fn acknowledge(&mut self, interrupt: u8) {
        self.if_ &= !interrupt;
        if interrupt & LCD_STAT_INTERRUPT != 0 { self.stat_pending = false; }
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0xFFFF => self.ie,
            0xFF0F => self.if_ | 0xE0, // Upper 3 bits unused, read as 1
            _ => 0,
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0xFFFF => self.ie = value,
            0xFF0F => self.if_ = value,
            _ => {}
        }
    }
}
