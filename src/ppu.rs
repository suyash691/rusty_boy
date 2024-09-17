pub struct PPU {
    vram: [u8; 8192],      // 8KB Video RAM
    oam: [u8; 160],        // Object Attribute Memory (40 sprites * 4 bytes each)
    lcd_control: u8,       // LCD Control register (0xFF40)
    lcd_status: u8,        // LCDC Status register (0xFF41)
    scroll_y: u8,          // Scroll Y register (0xFF42)
    scroll_x: u8,          // Scroll X register (0xFF43)
    ly: u8,                // LCD Y-Coordinate register (0xFF44)
    ly_compare: u8,        // LY Compare register (0xFF45)
    bg_palette: u8,        // BG Palette Data register (0xFF47)
    obj_palette0: u8,      // Object Palette 0 Data register (0xFF48)
    obj_palette1: u8,      // Object Palette 1 Data register (0xFF49)
    window_y: u8,          // Window Y Position register (0xFF4A)
    window_x: u8,          // Window X Position minus 7 register (0xFF4B)
    pub framebuffer: [u8; 160 * 144], // Stores the rendered frame (160x144 pixels)
    mode_clock: u32,       // Tracks cycles within the current PPU mode
}

impl PPU {
    pub fn new() -> Self {
        // Initialize PPU with default values
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
        }
    }

    pub fn update(&mut self, cycles: u32) {
        // Check if LCD is enabled
        if !self.is_lcd_enabled() {
            return;
        }

        // Add elapsed cycles to mode clock
        self.mode_clock += cycles;

        // Handle current PPU mode
        match self.get_mode() {
            0 => self.handle_hblank(),
            1 => self.handle_vblank(),
            2 => self.handle_oam_scan(),
            3 => self.handle_pixel_transfer(),
            _ => unreachable!(),
        }
    }

    fn is_lcd_enabled(&self) -> bool {
        // Check bit 7 of LCD Control register
        self.lcd_control & 0x80 != 0
    }

    fn get_mode(&self) -> u8 {
        // Get current mode from bits 0-1 of LCD Status register
        self.lcd_status & 0x03
    }

    fn set_mode(&mut self, mode: u8) {
        // Set mode in bits 0-1 of LCD Status register
        self.lcd_status = (self.lcd_status & 0xFC) | mode;
    }

    fn handle_hblank(&mut self) {
        if self.mode_clock >= 204 {  // HBlank period is 204 cycles
            self.mode_clock = 0;
            self.ly += 1;  // Move to next scanline

            if self.ly == 144 {
                self.set_mode(1);  // Switch to VBlank
                // TODO: Request VBlank interrupt
            } else {
                self.set_mode(2);  // Switch to OAM Scan
            }
        }
    }

    fn handle_vblank(&mut self) {
        if self.mode_clock >= 456 {  // Each scanline takes 456 cycles
            self.mode_clock = 0;
            self.ly += 1;

            if self.ly > 153 {  // VBlank period is 10 lines (144-153)
                self.ly = 0;
                self.set_mode(2);  // Switch to OAM Scan
            }
        }
    }

    fn handle_oam_scan(&mut self) {
        if self.mode_clock >= 80 {  // OAM Scan period is 80 cycles
            self.mode_clock = 0;
            self.set_mode(3);  // Switch to Pixel Transfer
        }
    }

    fn handle_pixel_transfer(&mut self) {
        if self.mode_clock >= 172 {  // Pixel Transfer period is 172 cycles
            self.mode_clock = 0;
            self.set_mode(0);  // Switch to HBlank
            self.render_scan_line();
        }
    }

    fn render_scan_line(&mut self) {
        // Render a single scanline
        let line = self.ly as usize;
        for x in 0..160 {
            let color = self.get_background_pixel(x, line);
            self.framebuffer[line * 160 + x] = color;
        }
    }

    fn get_background_pixel(&self, x: usize, y: usize) -> u8 {
        // Select the correct tile map area based on LCDC bit 3
        let tile_map_area = if self.lcd_control & 0x08 != 0 { 0x9C00 } else { 0x9800 };
        // Select the correct tile data area based on LCDC bit 4
        let tile_data_area = if self.lcd_control & 0x10 != 0 { 0x8000 } else { 0x8800 };

        // Apply scrolling
        let scroll_x = self.scroll_x as usize;
        let scroll_y = self.scroll_y as usize;
        let adjusted_x = (x + scroll_x) & 255;
        let adjusted_y = (y + scroll_y) & 255;

        // Find which tile the current pixel falls within
        let tile_x = adjusted_x / 8;
        let tile_y = adjusted_y / 8;

        // Get the tile index from the tile map
        let tile_index = self.vram[tile_map_area - 0x8000 + tile_y * 32 + tile_x];

        // Find the address of the tile data
        let tile_data_address = if tile_data_area == 0x8000 {
            // Unsigned addressing
            tile_data_area + (tile_index as usize) * 16
        } else {
            // Signed addressing
            (tile_data_area as usize).wrapping_add((tile_index as i8 as usize) * 16)
        };

        // Each tile occupies 16 bytes, 2 bytes per row
        let tile_row = (adjusted_y % 8) * 2;
        let tile_data_low = self.vram[tile_data_address - 0x8000 + tile_row];
        let tile_data_high = self.vram[tile_data_address - 0x8000 + tile_row + 1];

        // Extract the color value for this specific pixel
        let color_bit = 7 - (adjusted_x % 8);
        let color_num = ((tile_data_high >> color_bit) & 1) << 1 | ((tile_data_low >> color_bit) & 1);

        // Use the color number to get the actual color from the BG palette
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
            _ => panic!("Invalid PPU register address"),
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
            _ => panic!("Invalid PPU register address"),
        }
    }
}