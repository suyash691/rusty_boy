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
    /// `phase_lcd` at which the current STAT edge was RAISED (before the GH dispatch grid
    /// back-dated it). Used only for the HALT-exit timing model: hardware samples the un-halt
    /// (`halt_latch`, DELTA_CD) 4 phases EARLIER than the dispatch (`intf_latch`, DELTA_GH), so a
    /// halted CPU waking on a STAT raise that landed in the CD..GH window takes one extra M-cycle.
    pub stat_raise_phase: i64,
    /// One-shot guard for the HALT-exit deferral. `stat_raise_phase − stat_dispatch_phase` is a
    /// CONSTANT per raise, so the defer predicate would loop forever; this latches after we defer
    /// once and is cleared on the next raise / on acknowledge.
    pub stat_halt_deferred: bool,
}

impl InterruptController {
    pub fn new() -> Self {
        InterruptController { ie: 0, if_: 0, stat_raised: false, stat_pending: false,
            stat_publish_phase: 0, stat_dispatch_phase: 0, stat_raise_phase: 0,
            stat_halt_deferred: false }
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
    /// CPU-readable in `if_` at `publish_phase` (the read-latch). `raise_phase` is the raw raise
    /// (pre-grid) for the HALT-exit CD/GH timing model.
    pub fn request_stat(&mut self, dispatch_phase: i64, publish_phase: i64, raise_phase: i64) {
        self.stat_raised = true;
        self.stat_pending = false; // becomes dispatch-visible at the GH boundary
        self.stat_dispatch_phase = dispatch_phase;
        self.stat_publish_phase = publish_phase;
        self.stat_raise_phase = raise_phase;
        self.stat_halt_deferred = false; // a fresh edge re-arms the one-shot HALT deferral
    }

    /// HALT-exit timing: a halted CPU un-halts on the DELTA_CD IF sample, 4 phases (half an
    /// M-cycle) before the DELTA_GH dispatch sample our `pending()` models. So when a STAT raise
    /// lands in the CD..GH window — i.e. the GH grid back-dated it by ≥4 phases relative to the
    /// raw raise — the un-halt happens one M-cycle LATER than `pending()` suggests. True exactly
    /// for the raises that need `int_hblank_halt = int_hblank_nops + 1` (scx0/3/4/7). One-shot via
    /// `stat_halt_deferred` because the phase difference is constant per raise.
    pub fn stat_halt_should_defer(&self) -> bool {
        // Deciding-line (enable-line) measurement: HALT needs the +1 wake M-cycle exactly when the
        // raise-vs-GH-grid offset `(raise − dispatch) mod 8` is 0 or 6 — i.e. the raise sits on the
        // GH dispatch boundary (0) or 6 phases past it (just before the next CD un-halt sample).
        // diff ∈ {2,4} wake on time. Measured exact vs int_hblank_halt_scx0-7 (defer scx0/3/4/7).
        let diff = (self.stat_raise_phase - self.stat_dispatch_phase).rem_euclid(8);
        self.stat_pending && !self.stat_halt_deferred && (diff == 0 || diff == 6)
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
        if interrupt & LCD_STAT_INTERRUPT != 0 {
            self.stat_raised = false;
            self.stat_pending = false;
            self.stat_halt_deferred = false;
        }
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
