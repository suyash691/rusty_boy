use super::PPU;
use super::fifo::FifoPixel;

impl PPU {
    pub(super) fn check_sprite_at_x(&mut self) -> bool {
        for i in 0..self.sprite_count {
            let (_, sx, _, _) = self.sprite_buffer[i as usize];
            if sx as i16 == self.fifo.pixel_x {
                self.fifo.sprite_fetch_active = true;
                self.fifo.sprite_fetch_ticks = 0;
                self.fifo.sprite_fetch_penalty = 11u8.saturating_sub(sx & 7).max(6);
                self.fifo.sprite_oam_idx = i;
                // Mark sprite as consumed by setting x to 0xFF (won't match again)
                self.sprite_buffer[i as usize].1 = 0xFF;
                return true;
            }
        }
        false
    }

    pub(super) fn tick_sprite_fetch(&mut self) {
        self.fifo.sprite_fetch_ticks += 1;
        if self.fifo.sprite_fetch_ticks >= self.fifo.sprite_fetch_penalty {
            self.mix_sprite_into_fifo();
            self.fifo.sprite_fetch_active = false;
        }
    }

    fn mix_sprite_into_fifo(&mut self) {
        let idx = self.fifo.sprite_oam_idx as usize;
        if idx >= self.sprite_count as usize { return; }
        let (sy, _sx, mut tile, attrs) = self.sprite_buffer[idx];
        let flip_x = attrs & 0x20 != 0;
        let flip_y = attrs & 0x40 != 0;
        let behind_bg = attrs & 0x80 != 0;
        let tall = self.lcd_control & 0x04 != 0;
        let h: u8 = if tall { 16 } else { 8 };
        if tall { tile &= 0xFE; }

        let mut row = self.ly.wrapping_sub(sy) as usize;
        if flip_y { row = (h as usize - 1) - row; }

        let use_bank1 = self.cgb_mode && attrs & 0x08 != 0;
        let addr = tile as usize * 16 + row * 2;
        let (lo, hi) = if addr + 1 < 0x2000 {
            if use_bank1 { (self.vram_bank1[addr], self.vram_bank1[addr + 1]) }
            else { (self.vram[addr], self.vram[addr + 1]) }
        } else { (0, 0) };

        let pal = if self.cgb_mode { attrs & 0x07 }
        else if attrs & 0x10 != 0 { self.obj_palette1 } else { self.obj_palette0 };

        for bit in 0..8u8 {
            let fifo_bit = ((self.fifo.bg_fifo_head + bit) & 15) as usize;
            if bit >= self.fifo.bg_fifo_len { break; }
            let existing = &self.fifo.bg_fifo[fifo_bit];
            if existing.is_sprite { continue; }

            let pixel_bit = if flip_x { bit } else { 7 - bit };
            let color = ((hi >> pixel_bit) & 1) << 1 | ((lo >> pixel_bit) & 1);
            if color == 0 { continue; }
            if behind_bg && existing.color != 0 { continue; }
            if self.cgb_mode && existing.bg_priority && existing.color != 0 { continue; }

            self.fifo.bg_fifo[fifo_bit] = FifoPixel {
                color, palette: pal, is_sprite: true, bg_priority: false,
            };
        }
    }
}
