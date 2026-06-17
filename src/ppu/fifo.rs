use super::PPU;

#[derive(Clone, Copy, Default)]
pub(crate) struct FifoPixel {
    pub color: u8,
    pub palette: u8,
    pub is_sprite: bool,
    pub bg_priority: bool,
}

/// Fetcher states: 7 states, 1 dot each. PUSH stalls until FIFO is empty.
#[derive(Clone, Copy, PartialEq)]
enum FetcherState { GetTileT1, GetTileT2, GetDataLowT1, GetDataLowT2, GetDataHighT1, GetDataHighT2, Push }

pub struct PixelFifo {
    pub(crate) bg_fifo: [FifoPixel; 16],
    pub bg_fifo_len: u8,
    pub(crate) bg_fifo_head: u8,
    fetcher_state: FetcherState,
    pub(crate) fetch_x: u8,
    fetch_tile_id: u8,
    pub(crate) fetch_attrs: u8,
    fetch_data_low: u8,
    fetch_data_high: u8,
    /// Position in line: -16-SCX%8 to 160. Negative = discard. 0-159 = visible.
    pub pixel_x: i16,
    pub(crate) window_fetching: bool,
    pub(crate) window_x_counter: u8,
    pub(crate) sprite_fetch_active: bool,
    pub(crate) sprite_fetch_ticks: u8,
    pub(crate) sprite_fetch_penalty: u8,
    pub(crate) sprite_oam_idx: u8,
}

impl PixelFifo {
    pub fn new() -> Self {
        PixelFifo {
            bg_fifo: [FifoPixel::default(); 16], bg_fifo_len: 0, bg_fifo_head: 0,
            fetcher_state: FetcherState::GetTileT1,
            fetch_x: 0, fetch_tile_id: 0, fetch_attrs: 0,
            fetch_data_low: 0, fetch_data_high: 0,
            pixel_x: -8,
            window_fetching: false, window_x_counter: 0,
            sprite_fetch_active: false, sprite_fetch_ticks: 0, sprite_fetch_penalty: 6, sprite_oam_idx: 0,
        }
    }

    pub fn reset(&mut self, scx: u8) {
        *self = Self::new();
        // Start at -(8 + SCX%8): 8 junk pixels + SCX fractional scroll discarded
        self.pixel_x = -(8 + (scx & 7) as i16);
        // Pre-fill FIFO with 8 junk pixels (SameBoy model)
        self.bg_fifo_len = 8;
        self.bg_fifo_head = 0;
        for i in 0..8 { self.bg_fifo[i] = FifoPixel::default(); }
    }
}

impl PPU {
    /// One dot of mode 3: render pixel + advance fetcher
    pub(crate) fn tick_pixel_transfer(&mut self) -> bool {
        if self.fifo.sprite_fetch_active {
            self.tick_sprite_fetch();
            return false;
        }
        if self.lcd_control & 0x02 != 0 && self.check_sprite_at_x() {
            return false;
        }

        // Pop pixel from FIFO if available
        if self.fifo.bg_fifo_len > 0 {
            self.shift_pixel();
        }

        // Advance fetcher (1 state per dot)
        self.tick_fetcher();

        self.fifo.pixel_x >= 160
    }

    fn tick_fetcher(&mut self) {
        match self.fifo.fetcher_state {
            FetcherState::GetTileT1 => {
                self.fifo.fetcher_state = FetcherState::GetTileT2;
            }
            FetcherState::GetTileT2 => {
                let (map_base, tx, ty) = if self.fifo.window_fetching {
                    let map = if self.lcd_control & 0x40 != 0 { 0x1C00 } else { 0x1800 };
                    (map, self.fifo.window_x_counter as usize, (self.window_line / 8) as usize)
                } else {
                    let map = if self.lcd_control & 0x08 != 0 { 0x1C00 } else { 0x1800 };
                    let tx = ((self.scroll_x / 8) as usize + self.fifo.fetch_x as usize) & 31;
                    let ty = ((self.scroll_y as usize + self.ly as usize) / 8) & 31;
                    (map, tx, ty)
                };
                let offset = map_base + ty * 32 + tx;
                self.fifo.fetch_tile_id = self.vram[offset];
                self.fifo.fetch_attrs = if self.cgb_mode { self.vram_bank1[offset] } else { 0 };
                self.fifo.fetcher_state = FetcherState::GetDataLowT1;
            }
            FetcherState::GetDataLowT1 => {
                self.fifo.fetcher_state = FetcherState::GetDataLowT2;
            }
            FetcherState::GetDataLowT2 => {
                let addr = self.get_tile_row_addr(false);
                self.fifo.fetch_data_low = self.read_tile_byte(addr);
                self.fifo.fetcher_state = FetcherState::GetDataHighT1;
            }
            FetcherState::GetDataHighT1 => {
                self.fifo.fetcher_state = FetcherState::GetDataHighT2;
            }
            FetcherState::GetDataHighT2 => {
                let addr = self.get_tile_row_addr(true);
                self.fifo.fetch_data_high = self.read_tile_byte(addr);
                if self.fifo.window_fetching { self.fifo.window_x_counter += 1; }
                self.fifo.fetcher_state = FetcherState::Push;
            }
            FetcherState::Push => {
                // Push only when FIFO is empty (SameBoy: fifo_size == 0)
                if self.fifo.bg_fifo_len == 0 {
                    self.push_bg_row();
                    self.fifo.fetch_x += 1;
                    self.fifo.fetcher_state = FetcherState::GetTileT1;
                }
                // Otherwise stall in Push state
            }
        }
    }

    fn shift_pixel(&mut self) {
        let px = self.fifo.bg_fifo[self.fifo.bg_fifo_head as usize];
        self.fifo.bg_fifo_head = (self.fifo.bg_fifo_head + 1) & 15;
        self.fifo.bg_fifo_len -= 1;

        let pos = self.fifo.pixel_x;

        // Window trigger
        if !self.fifo.window_fetching && self.lcd_control & 0x20 != 0 && self.window_triggered {
            if pos >= 0 && pos as u8 == self.window_x.wrapping_sub(7) {
                self.fifo.window_fetching = true;
                self.fifo.bg_fifo_len = 0;
                self.fifo.bg_fifo_head = 0;
                self.fifo.fetcher_state = FetcherState::GetTileT1;
                self.fifo.window_x_counter = 0;
                self.fifo.pixel_x += 1;
                return;
            }
        }

        if pos >= 0 && pos < 160 {
            let x = pos as usize;
            let line = self.ly as usize;
            if line < 144 {
                if self.cgb_mode {
                    self.framebuffer[line * 160 + x] = (px.palette << 2) | px.color;
                } else {
                    let color = if px.is_sprite || (self.lcd_control & 0x01 != 0) {
                        (px.palette >> (px.color * 2)) & 0x03
                    } else { 0 };
                    self.framebuffer[line * 160 + x] = color;
                }
            }
        }
        self.fifo.pixel_x += 1;
    }

    fn get_tile_row_addr(&self, high: bool) -> usize {
        let attrs = self.fifo.fetch_attrs;
        let y_flip = self.cgb_mode && attrs & 0x40 != 0;
        let mut row = if self.fifo.window_fetching { (self.window_line % 8) as usize }
        else { (self.scroll_y as usize + self.ly as usize) % 8 };
        if y_flip { row = 7 - row; }
        self.tile_data_addr(self.fifo.fetch_tile_id) + row * 2 + if high { 1 } else { 0 }
    }

    fn read_tile_byte(&self, addr: usize) -> u8 {
        if addr >= 0x2000 { return 0; }
        if self.cgb_mode && self.fifo.fetch_attrs & 0x08 != 0 { self.vram_bank1[addr] }
        else { self.vram[addr] }
    }

    fn push_bg_row(&mut self) {
        let lo = self.fifo.fetch_data_low;
        let hi = self.fifo.fetch_data_high;
        let attrs = self.fifo.fetch_attrs;
        let x_flip = self.cgb_mode && attrs & 0x20 != 0;
        let cgb_pal = attrs & 0x07;
        let bg_prio = self.cgb_mode && attrs & 0x80 != 0;

        for i in 0..8u8 {
            let bit = if x_flip { i } else { 7 - i };
            let color = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);
            let idx = ((self.fifo.bg_fifo_head + self.fifo.bg_fifo_len) & 15) as usize;
            self.fifo.bg_fifo[idx] = FifoPixel {
                color,
                palette: if self.cgb_mode { cgb_pal } else { self.bg_palette },
                is_sprite: false,
                bg_priority: bg_prio,
            };
            self.fifo.bg_fifo_len += 1;
        }
    }

    fn tile_data_addr(&self, tile_id: u8) -> usize {
        if self.lcd_control & 0x10 != 0 { tile_id as usize * 16 }
        else { (0x1000i16 + (tile_id as i8 as i16) * 16) as usize }
    }
}
