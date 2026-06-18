use super::PPU;

impl PPU {
    pub(super) fn is_lcd_enabled(&self) -> bool { self.lcd_control & 0x80 != 0 }

    pub(super) fn set_mode(&mut self, mode: u8) {
        // Rising edge into HBlank (mode 0) — used to service HBlank HDMA once per line.
        if mode == 0 && self.current_mode != 0 { self.hblank_entered = true; }
        self.current_mode = mode;
        self.lcd_status = (self.lcd_status & 0xFC) | mode;
    }

    /// Returns the effective LY value for LYC comparison
    pub fn effective_ly(&self) -> u8 {
        if self.ly == 153 && self.ly_153_early_zero { 0 } else { self.ly }
    }

    pub(crate) fn check_lyc(&mut self) {
        let eff_ly = self.effective_ly();
        if eff_ly == self.ly_compare { self.lcd_status |= 0x04; }
        else { self.lcd_status &= !0x04; }
        self.update_stat_line();
    }

    /// VBlank entry (line 144): evaluates the STAT sources. Mode 1 (now current),
    /// LYC, and the mode-2/OAM source all apply here — the OAM source because the
    /// line-start strobe (`oam_stat_strobe`) also pulses at VBlank entry, exactly as on
    /// hardware (GateBoy TAPA_INT_OAM fires at the line-144 line-start). So this is just
    /// the normal STAT-line evaluation; no bespoke term needed.
    pub(super) fn check_vblank_stat(&mut self) {
        self.check_lyc(); // sets coincidence bit, then calls update_stat_line
    }

    /// DMG STAT-write IRQ bug: writing ANY value to $FF41 momentarily drives the STAT
    /// interrupt line as if ALL four source-enable bits were set, for that write cycle.
    /// If the current PPU condition (mode 0 HBlank, mode 2/OAM line-start strobe, mode 1
    /// VBlank, or LYC match) makes that all-enabled line high while the real `stat_line`
    /// was low, the spurious 0->1 edge latches a STAT IRQ. Called on the $FF41 write
    /// BEFORE the new enable bits are applied; leaves `stat_line` untouched so the normal
    /// evaluation that follows still sees the true (post-write) enables. Reproduces
    /// gbmicrotest `stat_write_glitch_*` — the E2/E0 dot edges emerge from where the mode
    /// conditions sit, not a tuned constant.
    pub(crate) fn stat_write_glitch(&mut self) {
        if !self.is_lcd_enabled() { return; }
        // Condition with ALL enables forced on (the bug pulse). Mode-0 (HBlank) uses the
        // 1-dot-delayed view (`current_mode == 0 && prev_mode == 0`) — the mode-3→0
        // transition the CPU observes lags the internal mode edge by one dot, the same
        // boundary the OAM/VRAM read-lock uses (gbmicrotest stat_write_glitch_l1_a vs _b
        // bracket it at dot 252 vs 256).
        let lyc_match = self.lcd_status & 0x04 != 0;
        let mode_0 = self.current_mode == 0 && self.prev_mode == 0 && self.mode3_done;
        let mode_1 = self.current_mode == 1;
        let mode_2 = self.oam_stat_strobe;
        let glitch_high = lyc_match || mode_0 || mode_1 || mode_2;
        if glitch_high && !self.stat_line {
            self.stat_interrupt = true;
        }
    }

    /// STAT blocking: only fire interrupt on rising edge of the combined STAT line
    pub(crate) fn update_stat_line(&mut self) {
        // With the LCD off the PPU produces no STAT interrupts; force the line low.
        if !self.is_lcd_enabled() {
            self.stat_line = false;
            return;
        }
        let lyc_match = self.lcd_status & 0x04 != 0 && self.lcd_status & 0x40 != 0;
        // Mode-0 STAT asserts only on a REAL HBlank (after mode 3 completed this line),
        // matching MetroBoy's WODU_HBLANK (pix_count==167) — NOT the enable line's
        // leading mode 0, which precedes any rendering (`mode3_done` is false there).
        let mode_0 = self.current_mode == 0 && self.mode3_done && self.lcd_status & 0x08 != 0;
        let mode_1 = self.current_mode == 1 && self.lcd_status & 0x10 != 0;
        // The mode-2/OAM STAT source is the line-start strobe (not a mode-2 level) — it
        // pulses at the top of each visible line and at VBlank entry (GateBoy TAPA_INT_OAM).
        let mode_2 = self.oam_stat_strobe && self.lcd_status & 0x20 != 0;

        let new_line = lyc_match || mode_0 || mode_1 || mode_2;
        if new_line && !self.stat_line {
            self.stat_interrupt = true;
        }
        self.stat_line = new_line;
    }

    pub(super) fn oam_scan(&mut self) {
        self.sprite_count = 0;
        let h: u8 = if self.lcd_control & 0x04 != 0 { 16 } else { 8 };
        for i in 0..40 {
            if self.sprite_count >= 10 { break; }
            let b = i * 4;
            let y = self.oam[b].wrapping_sub(16);
            if self.ly >= y && self.ly < y.wrapping_add(h) {
                self.sprite_buffer[self.sprite_count as usize] = (y, self.oam[b+1].wrapping_sub(8), self.oam[b+2], self.oam[b+3]);
                self.sprite_count += 1;
            }
        }
    }
}
