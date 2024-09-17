use std::fs::File;
use std::io::Read;

pub struct MMU {
    bios: [u8; 256],
    rom: Vec<u8>,
    eram: [u8; 8192],
    wram: [u8; 8192],
    zram: [u8; 127],
    in_bios: bool,
    ie: u8,
    if_: u8,
}

impl MMU {
    pub fn new() -> Self {
        MMU {
            bios: [
                0x31, 0xFE, 0xFF, 0xAF, 0x21, 0xFF, 0x9F, 0x32, 0xCB, 0x7C, 0x20, 0xFB, 0x21, 0x26, 0xFF, 0x0E,
                0x11, 0x3E, 0x80, 0x32, 0xE2, 0x0C, 0x3E, 0xF3, 0xE2, 0x32, 0x3E, 0x77, 0x77, 0x3E, 0xFC, 0xE0,
                0x47, 0x11, 0x04, 0x01, 0x21, 0x10, 0x80, 0x1A, 0xCD, 0x95, 0x00, 0xCD, 0x96, 0x00, 0x13, 0x7B,
                0xFE, 0x34, 0x20, 0xF3, 0x11, 0xD8, 0x00, 0x06, 0x08, 0x1A, 0x13, 0x22, 0x23, 0x05, 0x20, 0xF9,
                0x3E, 0x19, 0xEA, 0x10, 0x99, 0x21, 0x2F, 0x99, 0x0E, 0x0C, 0x3D, 0x28, 0x08, 0x32, 0x0D, 0x20,
                0xF9, 0x2E, 0x0F, 0x18, 0xF3, 0x67, 0x3E, 0x64, 0x57, 0xE0, 0x42, 0x3E, 0x91, 0xE0, 0x40, 0x04,
                0x1E, 0x02, 0x0E, 0x0C, 0xF0, 0x44, 0xFE, 0x90, 0x20, 0xFA, 0x0D, 0x20, 0xF7, 0x1D, 0x20, 0xF2,
                0x0E, 0x13, 0x24, 0x7C, 0x1E, 0x83, 0xFE, 0x62, 0x28, 0x06, 0x1E, 0xC1, 0xFE, 0x64, 0x20, 0x06,
                0x7B, 0xE2, 0x0C, 0x3E, 0x87, 0xF2, 0xF0, 0x42, 0x90, 0xE0, 0x42, 0x15, 0x20, 0xD2, 0x05, 0x20,
                0x4F, 0x16, 0x20, 0x18, 0xCB, 0x4F, 0x06, 0x04, 0xC5, 0xCB, 0x11, 0x17, 0xC1, 0xCB, 0x11, 0x17,
                0x05, 0x20, 0xF5, 0x22, 0x23, 0x22, 0x23, 0xC9, 0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B,
                0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D, 0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E,
                0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99, 0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC,
                0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E, 0x3c, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x4C,
                0x21, 0x04, 0x01, 0x11, 0xA8, 0x00, 0x1A, 0x13, 0xBE, 0x20, 0xFE, 0x23, 0x7D, 0xFE, 0x34, 0x20,
                0xF5, 0x06, 0x19, 0x78, 0x86, 0x23, 0x05, 0x20, 0xFB, 0x86, 0x20, 0xFE, 0x3E, 0x01, 0xE0, 0x50
            ],
            rom: Vec::new(),
            eram: [0; 8192],
            wram: [0; 8192],
            zram: [0; 127],
            in_bios: true,
            ie: 0,
            if_: 0,
        }
    }

    pub fn reset(&mut self) {
        self.eram = [0; 8192];
        self.wram = [0; 8192];
        self.zram = [0; 127];
        self.in_bios = true;
        self.ie = 0;
        self.if_ = 0;
    }

    pub fn load(&mut self, filename: &str) -> std::io::Result<()> {
        let mut file = File::open(filename)?;
        self.rom.clear();
        file.read_to_end(&mut self.rom)?;
        Ok(())
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr & 0xF000 {
            0x0000 => {
                if self.in_bios {
                    if addr < 0x0100 {
                        return self.bios[addr as usize];
                    }
                    // Note: We can't check Z80._r.pc here, so this part is omitted
                    // You might want to handle this differently in your implementation
                }
                if (self.rom.len()> 0)
                {
                    self.rom[addr as usize]
                }
                else
                {
                    0
                }
            }
            0x1000..=0x3000 => self.rom[addr as usize],
            0x4000..=0x7000 => self.rom[addr as usize],
            0x8000..=0x9000 => {
                // This should be handled by GPU._vram
                // For now, we'll return 0
                0
            }
            0xA000..=0xB000 => self.eram[(addr & 0x1FFF) as usize],
            0xC000..=0xE000 => self.wram[(addr & 0x1FFF) as usize],
            0xF000 => {
                match addr & 0x0F00 {
                    0x000..=0xD00 => self.wram[(addr & 0x1FFF) as usize],
                    0xE00 => {
                        if (addr & 0xFF) < 0xA0 {
                            // This should be handled by GPU._oam
                            // For now, we'll return 0
                            0
                        } else {
                            0
                        }
                    }
                    0xF00 => {
                        if addr > 0xFF7F {
                            self.zram[(addr & 0x7F) as usize]
                        } else {
                            // Handle I/O here
                            0
                        }
                    }
                    _ => 0,
                }
            }
            _ => 0,
        }
    }

    pub fn rw(&self, addr: u16) -> u16 {
        (self.read_byte(addr) as u16) | ((self.read_byte(addr + 1) as u16) << 8)
    }

    pub fn write_byte(&mut self, addr: u16, val: u8) {
        match addr & 0xF000 {
            0x0000..=0x7000 => {
                // ROM, read-only
            }
            0x8000..=0x9000 => {
                // This should update GPU._vram and call GPU.updatetile
                // For now, we'll do nothing
            }
            0xA000..=0xB000 => self.eram[(addr & 0x1FFF) as usize] = val,
            0xC000..=0xE000 => self.wram[(addr & 0x1FFF) as usize] = val,
            0xF000 => {
                match addr & 0x0F00 {
                    0x000..=0xD00 => self.wram[(addr & 0x1FFF) as usize] = val,
                    0xE00 => {
                        if (addr & 0xFF) < 0xA0 {
                            // This should update GPU._oam and call GPU.updateoam
                            // For now, we'll do nothing
                        }
                    }
                    0xF00 => {
                        if addr > 0xFF7F {
                            self.zram[(addr & 0x7F) as usize] = val;
                        } else {
                            // Handle I/O here
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub fn ww(&mut self, addr: u16, val: u16) {
        self.write_byte(addr, (val & 255) as u8);
        self.write_byte(addr + 1, (val >> 8) as u8);
    }
}