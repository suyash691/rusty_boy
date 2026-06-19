pub const VBLANK_INTERRUPT: u8 = 1 << 0;
pub const LCD_STAT_INTERRUPT: u8 = 1 << 1;
pub const TIMER_INTERRUPT: u8 = 1 << 2;
pub const SERIAL_INTERRUPT: u8 = 1 << 3;
pub const JOYPAD_INTERRUPT: u8 = 1 << 4;

pub struct InterruptController {
    pub ie: u8,
    pub if_: u8,
    /// STAT IRQ timing latches (two distinct phases — dispatch vs $FF0F read):
    /// - `stat_raised`: a PPU STAT edge occurred, not yet consumed.
    /// - `stat_pending`: DISPATCH-visible (ORed into `pending()`); becomes true once
    ///   `phase_lcd` reaches `stat_dispatch_phase` — the GH M-cycle boundary on hardware's
    ///   dispatch grid. Hardware latches IF for dispatch at phase GH, grouping all raises in
    ///   one M-cycle to one boundary (gbmicrotest hblank_int_scx*/int_hblank_*: the grid is
    ///   `floor((raise_dot-1)/4)`, 1 dot offset from a naive per-M-cycle settle).
    /// - `stat_publish_phase`: when the bit becomes CPU-READABLE in `if_` (the separate
    ///   $FF0F read-latch, raise + STAT_IF_DELAY) — gbmicrotest hblank_int_*_if_*.
    pub stat_raised: bool,
    pub stat_pending: bool,
    pub stat_publish_phase: i64,
    pub stat_dispatch_phase: i64,
}

impl InterruptController {
    pub fn new() -> Self {
        InterruptController { ie: 0, if_: 0, stat_raised: false, stat_pending: false,
            stat_publish_phase: 0, stat_dispatch_phase: 0 }
    }

    pub fn pending(&self) -> u8 {
        // Dispatch sees the STAT edge once it reaches its GH dispatch boundary; the
        // CPU-readable `if_` lags further by the read-latch delay.
        let if_eff = self.if_ | if self.stat_pending { LCD_STAT_INTERRUPT } else { 0 };
        self.ie & if_eff
    }

    pub fn request(&mut self, interrupt: u8) {
        self.if_ |= interrupt;
    }

    /// Mark a STAT edge raised: dispatch-visible at `dispatch_phase` (the GH boundary),
    /// CPU-readable in `if_` at `publish_phase` (the read-latch).
    pub fn request_stat(&mut self, dispatch_phase: i64, publish_phase: i64) {
        self.stat_raised = true;
        self.stat_pending = false; // becomes dispatch-visible at the GH boundary
        self.stat_dispatch_phase = dispatch_phase;
        self.stat_publish_phase = publish_phase;
    }

    /// Make a raised STAT edge dispatch-visible once `phase_lcd` reaches its GH boundary.
    pub fn commit_stat_dispatch(&mut self, phase_lcd: i64) {
        if self.stat_raised && phase_lcd >= self.stat_dispatch_phase {
            self.stat_pending = true;
        }
    }

    /// Commit a pending STAT edge into the CPU-readable `if_` once its publish phase passes.
    pub fn commit_stat_if(&mut self, phase_lcd: i64) {
        if self.stat_raised && phase_lcd >= self.stat_publish_phase {
            self.if_ |= LCD_STAT_INTERRUPT;
            self.stat_raised = false;
            self.stat_pending = false;
        }
    }

    pub fn acknowledge(&mut self, interrupt: u8) {
        self.if_ &= !interrupt;
        if interrupt & LCD_STAT_INTERRUPT != 0 { self.stat_raised = false; self.stat_pending = false; }
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
