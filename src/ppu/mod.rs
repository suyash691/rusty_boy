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
    pub(crate) mode_clock: u32,
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
    /// First scanline after LCD-on: its mode 2 (OAM) is shorter than normal, so
    /// HBlank arrives early — but the line still spans 456 dots (mode 0 absorbs the
    /// difference), so line 1+ start on time and steady-state timing is unaffected.
    pub(crate) first_line_after_on: bool,
}

/// On the first scanline after LCD-on, mode 2 ends this many dots earlier than the
/// usual 80, shifting HBlank earlier without changing the 456-dot line length.
pub(crate) const FIRST_LINE_MODE2_LEN: u32 = 77;

impl PPU {
    pub fn new() -> Self {
        PPU {
            vram: [0; 8192], vram_bank1: [0; 8192], vram_bank_num: 0, oam: [0; 160],
            lcd_control: 0, lcd_status: 0, scroll_y: 0, scroll_x: 0,
            ly: 0, ly_compare: 0, bg_palette: 0, obj_palette0: 0, obj_palette1: 0,
            window_y: 0, window_x: 0,
            framebuffer: [0; 160 * 144], mode_clock: 0, current_mode: 2,
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
            first_line_after_on: false,
        }
    }

    pub fn update(&mut self, cycles: u32) {
        if !self.is_lcd_enabled() { return; }

        match self.current_mode {
            2 => {
                self.mode_clock += cycles;
                // First line after LCD-on uses a shorter mode 2 so HBlank comes early;
                // the line still totals 456 dots, so steady state is unaffected.
                let mode2_len = if self.first_line_after_on { FIRST_LINE_MODE2_LEN } else { 80 };
                if self.mode_clock >= mode2_len {
                    self.oam_scan();
                    self.fifo.reset(self.scroll_x);
                    if self.lcd_control & 0x20 != 0 && !self.window_triggered && self.ly == self.window_y {
                        self.window_triggered = true;
                    }
                    self.set_mode(3);
                }
            }
            3 => {
                // Mode 3's first 4 dots are setup (fetcher idle). The gate is relative
                // to when mode 3 began (mode-2 length + 4), so it stays correct when the
                // first line uses a shorter mode 2.
                let mode2_len = if self.first_line_after_on { FIRST_LINE_MODE2_LEN } else { 80 };
                let setup_end = mode2_len + 4;
                for _ in 0..cycles {
                    self.mode_clock += 1;
                    if self.mode_clock <= setup_end { continue; }
                    if self.tick_pixel_transfer() {
                        if self.fifo.window_fetching { self.window_line += 1; }
                        self.set_mode(0);
                        self.update_stat_line();
                        break;
                    }
                }
            }
            0 => {
                self.mode_clock += cycles;
                if self.mode_clock >= 456 {
                    self.mode_clock -= 456;
                    self.ly += 1;
                    // Line 0 has ended; subsequent lines use normal mode-2 length.
                    self.first_line_after_on = false;
                    if self.ly == 144 {
                        self.set_mode(1);
                        self.vblank_interrupt = true;
                        // VBlank also triggers Mode 2 OAM STAT source (hardware quirk)
                        self.check_vblank_stat();
                        self.window_line = 0;
                    } else {
                        self.set_mode(2);
                        self.check_lyc();
                        self.update_stat_line();
                    }
                }
            }
            1 => {
                self.mode_clock += cycles;
                if self.mode_clock >= 456 {
                    self.mode_clock -= 456;
                    self.ly += 1;
                    if self.ly > 153 {
                        self.ly = 0;
                        self.set_mode(2);
                        self.check_lyc();
                        self.update_stat_line();
                        self.window_triggered = false;
                    } else if self.ly == 153 {
                        // Line 153: LY reads as 153 for ~4 dots, then becomes 0
                        // We handle this by setting ly_153_early_zero after a short delay
                        self.ly_153_early_zero = false;
                        self.check_lyc();
                    } else {
                        self.check_lyc();
                    }
                } else if self.ly == 153 && !self.ly_153_early_zero && self.mode_clock >= 4 {
                    // After 4 dots on line 153, LY reads as 0
                    self.ly_153_early_zero = true;
                    self.check_lyc(); // Re-evaluate with effective LY=0
                }
            }
            _ => unreachable!(),
        }
    }
}
