pub(crate) mod fifo;
mod io;
mod modes;
mod sprites;

pub use fifo::PixelFifo;

pub struct PPU {
    pub(crate) vram: [u8; 8192],
    pub(crate) vram_bank1: [u8; 8192],
    pub(crate) vram_bank_num: u8,
    pub(crate) oam: [u8; 160],
    pub(crate) lcd_control: u8,
    pub(crate) lcd_status: u8,
    pub(crate) scroll_y: u8,
    pub(crate) scroll_x: u8,
    pub(crate) ly: u8,
    pub(crate) ly_compare: u8,
    pub(crate) bg_palette: u8,
    pub(crate) obj_palette0: u8,
    pub(crate) obj_palette1: u8,
    pub(crate) window_y: u8,
    pub(crate) window_x: u8,
    pub(crate) framebuffer: [u8; 160 * 144],
    pub(crate) current_mode: u8,
    pub vblank_interrupt: bool,
    pub stat_interrupt: bool,
    pub(crate) stat_line: bool,
    pub(crate) hblank_entered: bool,
    pub dma_active: bool,
    pub dma_source: u16,
    pub dma_offset: u8,
    pub dma_delay: u8,
    pub(crate) window_line: u8,
    pub(crate) window_triggered: bool,
    pub(crate) sprite_buffer: [(u8, u8, u8, u8); 10],
    pub(crate) sprite_count: u8,
    pub(crate) fifo: PixelFifo,
    pub(crate) bg_cram: [u8; 64],
    pub(crate) obj_cram: [u8; 64],
    pub(crate) bg_cram_index: u8,
    pub(crate) bg_cram_auto_inc: bool,
    pub(crate) obj_cram_index: u8,
    pub(crate) obj_cram_auto_inc: bool,
    pub cgb_mode: bool,
    pub(crate) ly_153_early_zero: bool,
    /// Free-running LCD phase counter (MetroBoy mechanism — see
    /// rusty-boy-ppu-metroboy-port memory + docs/dmg-spec.md). Unit: MetroBoy "phases",
    /// 2 phases = 1 dot, 912 phases = 1 line. Pinned to 0 while the LCD is off; on
    /// software enable it restarts at phase 8 (the "PPU late by 1 M-cycle" glitch). LY,
    /// LX, mode boundaries, vblank, and the LY=153 early-zero are all DERIVED from it,
    /// so no magic constants are needed.
    pub(crate) phase_lcd: i64,
    /// Set true when mode 3 is entered (rendering) on the current line; cleared at line
    /// end. Drives the FIFO-based pixel transfer that determines mode-3 length.
    pub(crate) rendering: bool,
    /// Dots elapsed since mode 3 began this line (fetcher warm-up gate).
    pub(crate) mode3_dot: i32,
    /// True once mode 3 has run on the current line (reset at each LY edge), so a line
    /// renders at most once — needed because the enable line's pre-mode3 mode (0) equals
    /// its post-mode3 HBlank mode.
    pub(crate) mode3_done: bool,
    /// True only on the first scanline after a genuine *software* LCD enable (off→on):
    /// that line has NO mode 2 (starts in mode 0 → straight to mode 3). It is NOT set at
    /// boot — the boot handoff (see `boot_init`) produces a NORMAL line 0 with mode 2,
    /// just preceded by a VBlank residue. Distinguishing the two is essential: both land
    /// at `phase_lcd < 912`, so a phase-derived `first_line` guess conflates them.
    pub(crate) enable_quirk: bool,
}

/// Phases per line (MetroBoy). 912 phases = 456 dots. We advance 2 phases per dot.
const PHASES_PER_LINE: i64 = 912;
const PHASES_PER_FRAME: i64 = 154 * PHASES_PER_LINE;
/// Boot-handoff seed for `phase_lcd`: late in VBlank line 153, so LY reads 0
/// (early-zero) and STAT shows mode 1 (the poweron_* residue), then wraps into a
/// normal line 0 with OAM at the documented offset. Tuned against gbmicrotest poweron_*.
const BOOT_PHASE: i64 = 153 * PHASES_PER_LINE + 800;

impl PPU {
    pub fn new() -> Self {
        PPU {
            vram: [0; 8192], vram_bank1: [0; 8192], vram_bank_num: 0, oam: [0; 160],
            lcd_control: 0, lcd_status: 0, scroll_y: 0, scroll_x: 0,
            ly: 0, ly_compare: 0, bg_palette: 0, obj_palette0: 0, obj_palette1: 0,
            window_y: 0, window_x: 0,
            framebuffer: [0; 160 * 144], current_mode: 2,
            vblank_interrupt: false, stat_interrupt: false, stat_line: false, hblank_entered: false,
            dma_active: false, dma_source: 0, dma_offset: 0, dma_delay: 0,
            window_line: 0, window_triggered: false,
            sprite_buffer: [(0, 0, 0, 0); 10], sprite_count: 0,
            fifo: PixelFifo::new(),
            bg_cram: [0xFF; 64], obj_cram: [0xFF; 64],
            bg_cram_index: 0, bg_cram_auto_inc: false,
            obj_cram_index: 0, obj_cram_auto_inc: false,
            cgb_mode: false,
            ly_153_early_zero: false,
            phase_lcd: 0,
            rendering: false,
            mode3_dot: 0,
            mode3_done: false,
            enable_quirk: false,
        }
    }

    /// Seed the PPU to the DMG boot-ROM handoff phase. The boot ROM ran the LCD ~60
    /// frames and hands off mid-frame: `poweron_*` anchors with LY reading 0 while the
    /// PPU is physically in late VBlank (line 153, early-zero → mode 1 residue), which
    /// then wraps into a NORMAL line 0 (mode 2 OAM). Distinct from the software enable
    /// quirk; called by apply_post_boot_state AFTER the LCDC write.
    pub fn boot_init(&mut self) {
        self.phase_lcd = BOOT_PHASE;
        self.ly = 153;
        self.ly_153_early_zero = true;
        self.current_mode = 1;
        self.lcd_status = (self.lcd_status & 0xFC) | 1;
        self.rendering = false;
        self.mode3_done = false;
        self.enable_quirk = false; // boot line 0 is a NORMAL line (has mode 2), not the quirk
    }

    pub fn update(&mut self, cycles: u32) {
        if !self.is_lcd_enabled() { return; }
        // The clock delivers one PHASE per call (2 phases = 1 dot); loop for cycles > 1.
        for _ in 0..cycles {
            self.tick_phase();
        }
    }

    /// Advance the PPU by one PHASE (1/8 M-cycle; 2 phases = 1 dot) using the
    /// phase-derived gate-level model. LY, the mode-2/mode-3 boundary, VBlank, and the
    /// LY=153 early-zero are all functions of the free-running `phase_lcd` counter, now
    /// advancing 1 phase/tick so odd-phase boundaries (the sub-dot OAM/VRAM lock edges
    /// GateBoy resolves) are representable. The per-DOT machinery (FIFO + sprite fetcher,
    /// `mode3_dot`) runs once every 2 phases (on the even/dot boundary), so mode-3 stays
    /// exactly 172 dots.
    ///
    /// The line ORIGIN is the LogicBoy gate value: mode 2 opens at the `lx == 2` edge
    /// (besu_scan_donen window [2,162)), scan-done/mode-3 at `lx 162` (normal) / `166`
    /// (enable line), leaving a 1-dot leading mode-0 sliver (lx 0..1) = the previous
    /// line's HBlank tail. This origin shift is what flips the OAM/VRAM lock-RELEASE
    /// group (`*_l1_c`, `lcdon_to_stat*`, `line_153_*`) to passing — measured +6 on
    /// gbmicrotest. (The bus access stays sampled at phase 6, the per-dot boundary:
    /// moving it to phase 7 regressed the lock tests, so it was NOT adopted — see the
    /// regression analysis in zazzy-dreaming-ocean.md.)
    fn tick_phase(&mut self) {
        // Advance one phase, wrapping at the frame boundary.
        self.phase_lcd += 1;
        if self.phase_lcd >= PHASES_PER_FRAME { self.phase_lcd -= PHASES_PER_FRAME; }

        let lx = (self.phase_lcd % PHASES_PER_LINE) as i32; // 0..911
        let new_ly = (self.phase_lcd / PHASES_PER_LINE) as u8; // 0..153
        let on_dot = self.phase_lcd % 2 == 0; // even phase = a dot boundary

        // LY edge: a new scanline began.
        if new_ly != self.ly {
            // The software-enable quirk applies only to its own line 0; once we leave
            // line 0 (or reach VBlank) it's a normal line again.
            if new_ly != 0 { self.enable_quirk = false; }
            self.ly = new_ly;
            self.mode3_done = false; // mode 3 happens at most once per line
            if self.ly == 144 {
                // Entered VBlank.
                self.set_mode(1);
                self.vblank_interrupt = true;
                self.check_vblank_stat(); // VBlank entry also evaluates the mode-2 OAM source.
                self.window_line = 0;
                self.window_triggered = false;
                self.ly_153_early_zero = false;
            } else if self.ly < 144 {
                // Start of a visible line. Mode 2 opens at the lx==2 edge (LogicBoy
                // besu_scan_donen window [2,162)), leaving a 1-dot leading mode-0
                // sliver (lx 0..1) = the previous line's HBlank tail.
                self.rendering = false;
                self.check_lyc();
                self.update_stat_line();
            } else {
                // A VBlank line (145..153).
                if self.ly == 153 { self.ly_153_early_zero = false; }
                self.check_lyc();
            }
        }

        // LY=153 early-zero: LY reads 0 once we pass phase 153*912 + 4.
        if self.ly == 153 && !self.ly_153_early_zero
            && self.phase_lcd >= 153 * PHASES_PER_LINE + 4
        {
            self.ly_153_early_zero = true;
            self.check_lyc(); // Re-evaluate LYC with effective LY=0.
        }

        // Visible-line rendering: mode 2 → mode 3 at the scan-done boundary, then the
        // FIFO drives pixel transfer until it signals mode-3 end (→ mode 0).
        if self.ly < 144 {
            // Mode 2 (OAM scan) opens at lx==2 — LogicBoy's besu_scan_donen window
            // [2,162). The software-enable quirk line 0 has NO mode 2 (it starts in
            // mode 0 → straight to mode 3), so it is skipped here.
            if lx == 2 && !self.enable_quirk && !self.mode3_done && self.current_mode != 2 {
                self.set_mode(2);
                self.rendering = false;
                self.update_stat_line();
            }
            // Scan-done / mode-3 entry at lx 162 (normal) or 166 (enable line, +4 phases).
            let scan_done_lx = if self.enable_quirk { 166 } else { 162 };
            // Normal line enters mode 3 from mode 2; the software-enable quirk line enters
            // from mode 0 (no mode 2). `mode3_done` ensures it happens once per line —
            // without it, the quirk line (whose pre-mode3 mode is 0, same as the post-mode3
            // HBlank) would re-enter mode 3 repeatedly. The boot line 0 is NOT a quirk line.
            let pre_mode3 = if self.enable_quirk { self.current_mode == 0 } else { self.current_mode == 2 };
            if !self.rendering && !self.mode3_done && pre_mode3 && lx >= scan_done_lx {
                self.enter_mode3();
            } else if self.rendering && self.current_mode == 3 && on_dot {
                // Mode 3's first 4 dots are fetcher warm-up (no pixel output yet); our
                // FIFO models the rest. mode3_dot counts dots since rendering began, so
                // it advances only on the dot boundary (every 2 phases).
                self.mode3_dot += 1;
                if self.mode3_dot > 4 && self.tick_pixel_transfer() {
                    if self.fifo.window_fetching { self.window_line += 1; }
                    self.rendering = false;
                    self.mode3_done = true;
                    self.set_mode(0);
                    self.update_stat_line();
                }
            }
        }
    }

    /// Enter mode 3 (rendering): OAM scan, FIFO reset, window-trigger latch.
    fn enter_mode3(&mut self) {
        self.oam_scan();
        self.fifo.reset(self.scroll_x);
        if self.lcd_control & 0x20 != 0 && !self.window_triggered && self.ly == self.window_y {
            self.window_triggered = true;
        }
        self.rendering = true;
        self.mode3_dot = 0;
        self.set_mode(3);
    }
}
