pub struct InterruptController {
    // Interrupt Enable (IE) register
    // Bits: 0-VBLANK, 1-LCD STAT, 2-TIMER, 3-SERIAL, 4-JOYPAD
    ie: u8,

    // Interrupt Flag (IF) register
    // Bits: 0-VBLANK, 1-LCD STAT, 2-TIMER, 3-SERIAL, 4-JOYPAD
    if_: u8,

    // Interrupt Master Enable flag
    ime: bool,
}

impl InterruptController {
    pub fn new() -> Self {
        InterruptController {
            ie: 0,
            if_: 0,
            ime: false,
        }
    }

    // Get the currently active interrupts
    pub fn get_interrupts(&self) -> u8 {
        // Return the bitwise AND of IE and IF
        // This gives us the interrupts that are both enabled and requested
        self.ie & self.if_
    }

    // Request an interrupt
    pub fn request_interrupt(&mut self, interrupt: u8) {
        // Set the corresponding bit in the IF register
        self.if_ |= interrupt;
    }

    // Acknowledge an interrupt (clear it after it's been handled)
    pub fn acknowledge_interrupt(&mut self, interrupt: u8) {
        // Clear the corresponding bit in the IF register
        self.if_ &= !interrupt;
    }

    // Enable or disable the Interrupt Master Enable flag
    pub fn set_ime(&mut self, value: bool) {
        self.ime = value;
    }

    // Check if interrupts are globally enabled
    pub fn are_interrupts_enabled(&self) -> bool {
        self.ime
    }

    // Read from an interrupt-related memory address
    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0xFFFF => self.ie,
            0xFF0F => self.if_,
            _ => panic!("Invalid interrupt controller address"),
        }
    }

    // Write to an interrupt-related memory address
    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0xFFFF => self.ie = value,
            0xFF0F => self.if_ = value,
            _ => panic!("Invalid interrupt controller address"),
        }
    }
}

// Constants for different types of interrupts
pub const VBLANK_INTERRUPT: u8 = 1 << 0;
pub const LCD_STAT_INTERRUPT: u8 = 1 << 1;
pub const TIMER_INTERRUPT: u8 = 1 << 2;
pub const SERIAL_INTERRUPT: u8 = 1 << 3;
pub const JOYPAD_INTERRUPT: u8 = 1 << 4;