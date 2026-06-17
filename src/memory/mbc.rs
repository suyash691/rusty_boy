use super::MMU;

impl MMU {
    pub(super) fn rom_read(&self, addr: u16) -> u8 {
        let offset = match addr {
            0x0000..=0x3FFF => {
                if self.mbc_mode == 1 && self.is_mbc1() {
                    ((self.ram_bank as usize) << 5) * 0x4000 + addr as usize
                } else { addr as usize }
            }
            0x4000..=0x7FFF => self.rom_bank.max(1) as usize * 0x4000 + (addr as usize - 0x4000),
            _ => addr as usize,
        };
        if offset < self.rom.len() { self.rom[offset] } else { 0xFF }
    }

    fn is_mbc1(&self) -> bool { matches!(self.mbc_type, 1..=3) }
    fn is_mbc3(&self) -> bool { matches!(self.mbc_type, 0x0F..=0x13) }
    fn is_mbc5(&self) -> bool { matches!(self.mbc_type, 0x19..=0x1E) }

    pub(super) fn mbc_write(&mut self, addr: u16, value: u8) {
        if self.mbc_type == 0 { return; }
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (value & 0x0F) == 0x0A,
            0x2000..=0x3FFF => {
                if self.is_mbc1() {
                    let mut b = (value & 0x1F) as u16; if b == 0 { b = 1; }
                    self.rom_bank = (self.rom_bank & 0x60) | b;
                } else if self.is_mbc3() {
                    let mut b = (value & 0x7F) as u16; if b == 0 { b = 1; }
                    self.rom_bank = b;
                } else if self.is_mbc5() {
                    if addr < 0x3000 { self.rom_bank = (self.rom_bank & 0x100) | value as u16; }
                    else { self.rom_bank = (self.rom_bank & 0xFF) | ((value as u16 & 1) << 8); }
                }
            }
            0x4000..=0x5FFF => {
                if self.is_mbc1() {
                    self.ram_bank = value & 0x03;
                    if self.mbc_mode == 0 { self.rom_bank = (self.rom_bank & 0x1F) | ((value as u16 & 0x03) << 5); }
                } else { self.ram_bank = value & 0x0F; }
            }
            0x6000..=0x7FFF => {
                if self.is_mbc1() { self.mbc_mode = value & 0x01; }
                else if self.is_mbc3() {
                    // RTC latch: writing 0x00 then 0x01 latches current time
                    if self.rtc_latch_prev == 0 && value == 1 { self.rtc_latch(); }
                    self.rtc_latch_prev = value;
                }
            }
            _ => {}
        }
    }

    pub(super) fn cart_ram_read(&self, addr: u16) -> u8 {
        if !self.ram_enabled { return 0xFF; }
        // MBC3 RTC registers mapped when ram_bank is 0x08-0x0C
        if self.is_mbc3() && self.ram_bank >= 0x08 {
            return self.rtc_read(self.ram_bank);
        }
        let offset = self.ram_bank as usize * 0x2000 + (addr as usize - 0xA000);
        if offset < self.cart_ram.len() { self.cart_ram[offset] } else { 0xFF }
    }

    pub(super) fn cart_ram_write(&mut self, addr: u16, value: u8) {
        if !self.ram_enabled { return; }
        if self.is_mbc3() && self.ram_bank >= 0x08 {
            self.rtc_write(self.ram_bank, value);
            return;
        }
        let offset = self.ram_bank as usize * 0x2000 + (addr as usize - 0xA000);
        if offset < self.cart_ram.len() { self.cart_ram[offset] = value; }
    }

    // --- RTC ---
    fn rtc_latch(&mut self) {
        self.rtc_latched = self.rtc_regs;
    }

    fn rtc_read(&self, reg: u8) -> u8 {
        match reg {
            0x08 => self.rtc_latched[0], // Seconds
            0x09 => self.rtc_latched[1], // Minutes
            0x0A => self.rtc_latched[2], // Hours
            0x0B => self.rtc_latched[3], // Days low
            0x0C => self.rtc_latched[4], // Days high + halt + carry
            _ => 0xFF,
        }
    }

    fn rtc_write(&mut self, reg: u8, value: u8) {
        match reg {
            0x08 => self.rtc_regs[0] = value & 0x3F,
            0x09 => self.rtc_regs[1] = value & 0x3F,
            0x0A => self.rtc_regs[2] = value & 0x1F,
            0x0B => self.rtc_regs[3] = value,
            0x0C => self.rtc_regs[4] = value & 0xC1,
            _ => {}
        }
    }

    /// Tick RTC - call periodically (e.g., once per frame)
    pub fn tick_rtc(&mut self) {
        if !self.is_mbc3() { return; }
        if self.rtc_regs[4] & 0x40 != 0 { return; } // Halted
        self.rtc_sub_seconds += 1;
        if self.rtc_sub_seconds < 60 { return; } // ~1 second per 60 frames
        self.rtc_sub_seconds = 0;
        self.rtc_regs[0] += 1;
        if self.rtc_regs[0] < 60 { return; }
        self.rtc_regs[0] = 0; self.rtc_regs[1] += 1;
        if self.rtc_regs[1] < 60 { return; }
        self.rtc_regs[1] = 0; self.rtc_regs[2] += 1;
        if self.rtc_regs[2] < 24 { return; }
        self.rtc_regs[2] = 0;
        let days = self.rtc_regs[3] as u16 | ((self.rtc_regs[4] as u16 & 1) << 8);
        let new_days = days + 1;
        self.rtc_regs[3] = new_days as u8;
        self.rtc_regs[4] = (self.rtc_regs[4] & 0xFE) | ((new_days >> 8) as u8 & 1);
        if new_days > 511 { self.rtc_regs[4] |= 0x80; } // Day carry
    }
}
