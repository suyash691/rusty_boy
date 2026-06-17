use super::PPU;

impl PPU {
    pub fn read_io(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcd_control, 0xFF41 => self.lcd_status | 0x80,
            0xFF42 => self.scroll_y, 0xFF43 => self.scroll_x,
            0xFF44 => self.effective_ly(), 0xFF45 => self.ly_compare,
            0xFF46 => (self.dma_source >> 8) as u8,
            0xFF47 => self.bg_palette, 0xFF48 => self.obj_palette0, 0xFF49 => self.obj_palette1,
            0xFF4A => self.window_y, 0xFF4B => self.window_x,
            _ => 0xFF,
        }
    }

    pub fn write_io(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF40 => {
                let was_on = self.lcd_control & 0x80 != 0;
                let now_on = value & 0x80 != 0;
                self.lcd_control = value;
                if was_on && !now_on {
                    // LCD turned off: blank, freeze the phase counter, mode reads 0.
                    self.ly = 0;
                    self.phase_lcd = 0;
                    self.rendering = false;
                    self.mode3_done = false;
                    self.current_mode = 0;
                    self.lcd_status &= 0xFC;
                    self.window_line = 0;
                    self.window_triggered = false;
                    self.stat_line = false;
                } else if !was_on && now_on {
                    // LCD turned on: MetroBoy enable-glitch — the phase counter restarts at
                    // phase 8 (= 4 dots / 1 M-cycle "late"), and line 0 is the `first_line`
                    // with no mode 2 (it starts in mode 0 → straight to mode 3). All of this
                    // falls out of phase_lcd=8 + the first_line branch in tick_dot.
                    self.ly = 0;
                    self.phase_lcd = 8;
                    self.rendering = false;
                    self.mode3_done = false;
                    self.current_mode = 0; // first_line begins in mode 0 (no OAM scan)
                    self.lcd_status = (self.lcd_status & 0xFC) | 0;
                    self.check_lyc();
                    self.update_stat_line();
                }
            }
            0xFF41 => {
                self.lcd_status = (self.lcd_status & 0x07) | (value & 0x78);
                self.update_stat_line();
            }
            0xFF42 => self.scroll_y = value, 0xFF43 => self.scroll_x = value,
            0xFF44 => {} 0xFF45 => { self.ly_compare = value; self.check_lyc(); }
            0xFF46 => { self.dma_source = (value as u16) << 8; self.dma_active = true; self.dma_offset = 0; self.dma_delay = 2; }
            0xFF47 => self.bg_palette = value, 0xFF48 => self.obj_palette0 = value, 0xFF49 => self.obj_palette1 = value,
            0xFF4A => self.window_y = value, 0xFF4B => self.window_x = value,
            _ => {}
        }
    }

    // --- VRAM banking (CGB) ---
    pub fn read_vbk(&self) -> u8 { self.vram_bank_num | 0xFE }
    pub fn write_vbk(&mut self, value: u8) { self.vram_bank_num = value & 1; }

    pub fn read_vram(&self, addr: u16) -> u8 {
        if self.current_mode == 3 { return 0xFF; }
        let offset = (addr - 0x8000) as usize;
        if self.vram_bank_num == 0 { self.vram[offset] } else { self.vram_bank1[offset] }
    }

    pub fn write_vram(&mut self, addr: u16, value: u8) {
        if addr < 0x8000 || addr > 0x9FFF { return; }
        let offset = (addr - 0x8000) as usize;
        if self.vram_bank_num == 0 { self.vram[offset] = value; }
        else { self.vram_bank1[offset] = value; }
    }

    // --- OAM access ---
    pub fn read_oam(&self, addr: u16) -> u8 {
        if self.current_mode >= 2 { 0xFF } else { self.oam[(addr - 0xFE00) as usize] }
    }

    pub fn write_oam(&mut self, addr: u16, value: u8) {
        if self.current_mode < 2 { self.oam[(addr - 0xFE00) as usize] = value; }
    }

    pub fn dma_write_oam(&mut self, byte: u8) {
        if self.dma_offset < 160 {
            self.oam[self.dma_offset as usize] = byte;
            self.dma_offset += 1;
            if self.dma_offset >= 160 { self.dma_active = false; }
        }
    }

    // --- CGB color palette RAM ---
    pub fn read_cgb_palette(&self, addr: u16) -> u8 {
        match addr {
            0xFF68 => self.bg_cram_index | (if self.bg_cram_auto_inc { 0x80 } else { 0 }) | 0x40,
            0xFF69 => self.bg_cram[self.bg_cram_index as usize & 0x3F],
            0xFF6A => self.obj_cram_index | (if self.obj_cram_auto_inc { 0x80 } else { 0 }) | 0x40,
            0xFF6B => self.obj_cram[self.obj_cram_index as usize & 0x3F],
            _ => 0xFF,
        }
    }

    pub fn write_cgb_palette(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF68 => { self.bg_cram_index = value & 0x3F; self.bg_cram_auto_inc = value & 0x80 != 0; }
            0xFF69 => { self.bg_cram[self.bg_cram_index as usize & 0x3F] = value; if self.bg_cram_auto_inc { self.bg_cram_index = (self.bg_cram_index + 1) & 0x3F; } }
            0xFF6A => { self.obj_cram_index = value & 0x3F; self.obj_cram_auto_inc = value & 0x80 != 0; }
            0xFF6B => { self.obj_cram[self.obj_cram_index as usize & 0x3F] = value; if self.obj_cram_auto_inc { self.obj_cram_index = (self.obj_cram_index + 1) & 0x3F; } }
            _ => {}
        }
    }

    pub fn cgb_color_to_rgb(&self, palette_ram: &[u8], palette_num: u8, color_id: u8) -> u32 {
        let idx = (palette_num as usize * 8 + color_id as usize * 2) & 0x3E;
        let rgb555 = palette_ram[idx] as u16 | ((palette_ram[idx + 1] as u16) << 8);
        let r = ((rgb555 & 0x1F) * 255 / 31) as u32;
        let g = (((rgb555 >> 5) & 0x1F) * 255 / 31) as u32;
        let b = (((rgb555 >> 10) & 0x1F) * 255 / 31) as u32;
        0xFF000000 | (r << 16) | (g << 8) | b
    }

    // --- Framebuffer output ---
    pub fn get_frame_buffer(&self) -> Vec<u32> {
        if self.cgb_mode {
            self.framebuffer.iter().map(|&packed| {
                self.cgb_color_to_rgb(&self.bg_cram, (packed >> 2) & 0x07, packed & 0x03)
            }).collect()
        } else {
            self.framebuffer.iter().map(|&c| match c & 0x03 {
                0 => 0xFFE0F8D0, 1 => 0xFF88C070, 2 => 0xFF346856, 3 => 0xFF081820, _ => 0xFFFF0000,
            }).collect()
        }
    }

    // --- Debug ---
    pub fn debug_mode(&self) -> u8 { self.current_mode }
    pub fn debug_ly(&self) -> u8 { self.ly }
    pub fn debug_pixel_x(&self) -> u8 { self.fifo.pixel_x.max(0) as u8 }
    pub fn debug_fifo_len(&self) -> u8 { self.fifo.bg_fifo_len }
    pub fn debug_vram(&self, offset: usize) -> u8 { self.vram[offset] }

    /// Raw framebuffer as 2-bit DMG shade indices (0 = lightest .. 3 = darkest).
    /// Palette-independent, for screenshot comparison against test references.
    pub fn framebuffer_shades(&self) -> Vec<u8> {
        self.framebuffer.iter().map(|&c| c & 0x03).collect()
    }
}
