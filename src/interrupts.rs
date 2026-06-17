pub const VBLANK_INTERRUPT: u8 = 1 << 0;
pub const LCD_STAT_INTERRUPT: u8 = 1 << 1;
pub const TIMER_INTERRUPT: u8 = 1 << 2;
pub const SERIAL_INTERRUPT: u8 = 1 << 3;
pub const JOYPAD_INTERRUPT: u8 = 1 << 4;

pub struct InterruptController {
    pub ie: u8,
    pub if_: u8,
}

impl InterruptController {
    pub fn new() -> Self {
        InterruptController { ie: 0, if_: 0 }
    }

    pub fn pending(&self) -> u8 {
        self.ie & self.if_
    }

    pub fn request(&mut self, interrupt: u8) {
        self.if_ |= interrupt;
    }

    pub fn acknowledge(&mut self, interrupt: u8) {
        self.if_ &= !interrupt;
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0xFFFF => self.ie,
            0xFF0F => self.if_ | 0xE0, // Upper 3 bits unused, read as 1
            _ => 0,
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0xFFFF => self.ie = value,
            0xFF0F => self.if_ = value,
            _ => {}
        }
    }
}
