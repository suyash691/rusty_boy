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

    /// VBlank entry: triggers Mode 1 and Mode 2 OAM STAT sources
    pub(super) fn check_vblank_stat(&mut self) {
        self.check_lyc();
        // On hardware, mode 2 OAM select (bit 5) also fires at VBlank entry
        let prev = self.stat_line;
        let lyc_match = self.lcd_status & 0x04 != 0 && self.lcd_status & 0x40 != 0;
        let mode_1 = self.lcd_status & 0x10 != 0;
        let mode_2_oam = self.lcd_status & 0x20 != 0;
        let new_line = lyc_match || mode_1 || mode_2_oam;
        if new_line && !prev {
            self.stat_interrupt = true;
        }
        self.stat_line = new_line;
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
        let mode_2 = self.current_mode == 2 && self.lcd_status & 0x20 != 0;

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
