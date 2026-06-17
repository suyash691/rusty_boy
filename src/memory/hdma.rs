use super::MMU;

impl MMU {
    pub(super) fn read_hdma(&self, addr: u16) -> u8 {
        match addr {
            0xFF51 => (self.hdma_src >> 8) as u8,
            0xFF52 => (self.hdma_src & 0xF0) as u8,
            0xFF53 => (self.hdma_dst >> 8) as u8,
            0xFF54 => (self.hdma_dst & 0xF0) as u8,
            0xFF55 => if self.hdma_active { self.hdma_len & 0x7F } else { 0xFF },
            _ => 0xFF,
        }
    }

    pub(super) fn write_hdma(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF51 => self.hdma_src = (self.hdma_src & 0x00FF) | ((value as u16) << 8),
            0xFF52 => self.hdma_src = (self.hdma_src & 0xFF00) | ((value as u16) & 0xF0),
            0xFF53 => self.hdma_dst = (self.hdma_dst & 0x00FF) | (((value as u16) & 0x1F) << 8) | 0x8000,
            0xFF54 => self.hdma_dst = (self.hdma_dst & 0xFF00) | ((value as u16) & 0xF0),
            0xFF55 => {
                if self.hdma_active && value & 0x80 == 0 {
                    self.hdma_active = false;
                } else {
                    self.hdma_len = value & 0x7F;
                    self.hdma_hblank = value & 0x80 != 0;
                    if self.hdma_hblank { self.hdma_active = true; }
                    else { self.execute_gdma(); }
                }
            }
            _ => {}
        }
    }

    fn execute_gdma(&mut self) {
        let len = (self.hdma_len as u16 + 1) * 16;
        for i in 0..len {
            let byte = self.read_byte(self.hdma_src.wrapping_add(i));
            self.ppu.write_vram(self.hdma_dst.wrapping_add(i), byte);
        }
        self.hdma_src = self.hdma_src.wrapping_add(len);
        self.hdma_dst = self.hdma_dst.wrapping_add(len);
        self.hdma_len = 0xFF;
        self.hdma_active = false;
    }

    pub fn tick_hdma(&mut self) {
        if !self.hdma_active || !self.hdma_hblank { return; }
        for i in 0..16u16 {
            let byte = self.read_byte(self.hdma_src.wrapping_add(i));
            self.ppu.write_vram(self.hdma_dst.wrapping_add(i), byte);
        }
        self.hdma_src = self.hdma_src.wrapping_add(16);
        self.hdma_dst = self.hdma_dst.wrapping_add(16);
        if self.hdma_len == 0 { self.hdma_active = false; self.hdma_len = 0xFF; }
        else { self.hdma_len -= 1; }
    }

    pub fn do_speed_switch(&mut self) {
        if self.speed_switch_armed {
            self.double_speed = !self.double_speed;
            self.speed_switch_armed = false;
        }
    }
}
