use crate::memory::MMU;

pub struct PPU {
    vram: [u8; 8192],
    oam: [u8; 160],
    lcd_control: u8,
    lcd_status: u8,
    scroll_y: u8,
    scroll_x: u8,
    ly: u8,
    ly_compare: u8,
    bg_palette: u8,
    obj_palette0: u8,
    obj_palette1: u8,
    window_y: u8,
    window_x: u8,
    pub framebuffer: [u8; 160 * 144],
    mode_clock: u32,
    current_mode: u8,
    dma_active: bool,
    dma_byte: u8,
    dma_start_delay: u8,
    dma_source: u16,
}

impl PPU {
    pub fn new() -> Self {
        PPU {
            vram: [0; 8192],
            oam: [0; 160],
            lcd_control: 0,
            lcd_status: 0,
            scroll_y: 0,
            scroll_x: 0,
            ly: 0,
            ly_compare: 0,
            bg_palette: 0,
            obj_palette0: 0,
            obj_palette1: 0,
            window_y: 0,
            window_x: 0,
            framebuffer: [0; 160 * 144],
            mode_clock: 0,
            current_mode: 2,
            dma_active: false,
            dma_byte: 0,
            dma_start_delay: 0,
            dma_source: 0,
        }
    }

    pub fn update(&mut self, cycles: u32) {
        if !self.is_lcd_enabled() {
            return;
        }

        self.mode_clock += cycles;

        match self.current_mode {
            2 => self.handle_oam_scan(),
            3 => self.handle_pixel_transfer(),
            0 => self.handle_hblank(),
            1 => self.handle_vblank(),
            _ => unreachable!(),
        }

        if self.dma_active {
            self.update_dma(cycles);
        }

        for y in 0..144 {
            for x in 0..160 {
                let color = ((x + y) % 4) as u8;
                self.framebuffer[y * 160 + x] = color;
            }
        }
        println!("PPU updated");
    }

    fn update_dma(&mut self, mut cycles: u32) {
        if self.dma_start_delay > 0 {
            if cycles >= self.dma_start_delay as u32 {
                cycles -= self.dma_start_delay as u32;
                self.dma_start_delay = 0;
            } else {
                self.dma_start_delay -= cycles as u8;
                return;
            }
        }

        let bytes_to_transfer = cycles.min(160 - self.dma_byte as u32);
        self.dma_byte += bytes_to_transfer as u8;

        if self.dma_byte >= 160 {
            self.dma_active = false;
        }
    }

    pub fn dma_read(&self, addr: u16) -> u8 {
        // This method should be called by MMU during DMA
        self.oam[addr as usize]
    }

    pub fn dma_write(&mut self, addr: u16, value: u8) {
        // This method should be called by MMU during DMA
        self.oam[addr as usize] = value;
    }

    fn is_lcd_enabled(&self) -> bool {
        self.lcd_control & 0x80 != 0
    }

    fn handle_oam_scan(&mut self) {
        if self.mode_clock >= 80 {
            self.mode_clock = 0;
            self.current_mode = 3;
            self.set_mode(3);
        }
    }

    fn handle_pixel_transfer(&mut self) {
        if self.mode_clock >= 172 {
            self.mode_clock = 0;
            self.current_mode = 0;
            self.set_mode(0);
            self.render_scan_line();
        }
    }

    fn handle_hblank(&mut self) {
        if self.mode_clock >= 204 {
            self.mode_clock = 0;
            self.ly += 1;

            if self.ly == 144 {
                self.current_mode = 1;
                self.set_mode(1);
                // Request VBlank interrupt
            } else {
                self.current_mode = 2;
                self.set_mode(2);
            }
        }
    }

    fn handle_vblank(&mut self) {
        if self.mode_clock >= 456 {
            self.mode_clock = 0;
            self.ly += 1;

            if self.ly > 153 {
                self.ly = 0;
                self.current_mode = 2;
                self.set_mode(2);
            }
        }
    }

    fn set_mode(&mut self, mode: u8) {
        self.lcd_status = (self.lcd_status & 0xFC) | mode;
    }

    fn render_scan_line(&mut self) {
        let line = self.ly as usize;
        for x in 0..160 {
            let color = self.get_background_pixel(x, line);
            self.framebuffer[line * 160 + x] = color;
        }
    }

    pub fn get_frame_buffer(&self) -> Vec<u32> {
        self.framebuffer.iter().map(|&color| {
            match color {
                0 => 0xFFFFFFFF, // White
                1 => 0xAAAAAAFF, // Light gray
                2 => 0x555555FF, // Dark gray
                3 => 0x000000FF, // Black
                _ => 0xFF0000FF, // Red (shouldn't happen)
            }
        }).collect()
    }

    fn get_background_pixel(&self, x: usize, y: usize) -> u8 {
        let tile_map = if self.lcd_control & 0x08 != 0 { 0x9C00 } else { 0x9800 };
        let tile_data = if self.lcd_control & 0x10 != 0 { 0x8000 } else { 0x8800 };

        let scroll_x = self.scroll_x as usize;
        let scroll_y = self.scroll_y as usize;

        let adjusted_x = (x + scroll_x) & 255;
        let adjusted_y = (y + scroll_y) & 255;

        let tile_x = adjusted_x / 8;
        let tile_y = adjusted_y / 8;

        let tile_index_addr = tile_map - 0x8000 + tile_y * 32 + tile_x;
        let tile_index = self.vram[tile_index_addr];

        let tile_data_addr = if tile_data == 0x8000 {
            tile_data - 0x8000 + (tile_index as usize) * 16
        } else {
            (tile_data - 0x8000 as usize).wrapping_add((tile_index as i8 as usize) * 16)
        };

        let tile_row = (adjusted_y % 8) * 2;
        let tile_data_low = self.vram[tile_data_addr + tile_row];
        let tile_data_high = self.vram[tile_data_addr + tile_row + 1];

        let color_bit = 7 - (adjusted_x % 8);
        let color_num = ((tile_data_high >> color_bit) & 1) << 1 | ((tile_data_low >> color_bit) & 1);

        (self.bg_palette >> (color_num * 2)) & 0x03
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFF40 => self.lcd_control,
            0xFF41 => self.lcd_status,
            0xFF42 => self.scroll_y,
            0xFF43 => self.scroll_x,
            0xFF44 => self.ly,
            0xFF45 => self.ly_compare,
            0xFF47 => self.bg_palette,
            0xFF48 => self.obj_palette0,
            0xFF49 => self.obj_palette1,
            0xFF4A => self.window_y,
            0xFF4B => self.window_x,
            0xFF46 => (self.dma_source >> 8) as u8,
            _ => panic!("Invalid PPU register address: {:04X}", addr),
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = value,
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = value,
            0xFF40 => self.lcd_control = value,
            0xFF41 => self.lcd_status = value,
            0xFF42 => self.scroll_y = value,
            0xFF43 => self.scroll_x = value,
            0xFF44 => (), // LY is read-only
            0xFF45 => self.ly_compare = value,
            0xFF47 => self.bg_palette = value,
            0xFF48 => self.obj_palette0 = value,
            0xFF49 => self.obj_palette1 = value,
            0xFF4A => self.window_y = value,
            0xFF4B => self.window_x = value,
            0xFF46 => self.start_dma(value),
            _ => panic!("Invalid PPU register address: {:04X}", addr),
        }
    }

    fn start_dma(&mut self, value: u8) {
        self.dma_source = (value as u16) << 8;
        self.dma_active = true;
        self.dma_byte = 0;
        self.dma_start_delay = 2;
    }
}