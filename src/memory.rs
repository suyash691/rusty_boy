use crate::ppu::PPU;
use crate::interrupts::InterruptController;
use std::fs::File;
use std::io::Read;

pub struct MMU {
    boot_rom: [u8; 256],
    rom: Vec<u8>,
    ram: [u8; 8192],
    zero_page: [u8; 127],
    in_boot: bool,
    ppu: PPU,
    interrupt_controller: InterruptController,
}

impl MMU {
    pub fn new() -> Self {
        MMU {
            boot_rom: [0; 256],
            rom: Vec::new(),
            ram: [0; 8192],
            zero_page: [0; 127],
            in_boot: true,
            ppu: PPU::new(),
            interrupt_controller: InterruptController::new(),
        }
    }

    pub fn load_boot_rom(&mut self, filename: &str) -> std::io::Result<()> {
        let mut file = File::open(filename)?;
        file.read_exact(&mut self.boot_rom)?;
        Ok(())
    }

    pub fn load_rom(&mut self, filename: &str) -> std::io::Result<()> {
        let mut file = File::open(filename)?;
        self.rom.clear();
        file.read_to_end(&mut self.rom)?;
        Ok(())
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00FF if self.in_boot => self.boot_rom[addr as usize],
            0x0000..=0x7FFF => self.rom[addr as usize],
            0x8000..=0x9FFF => self.ppu.read_byte(addr),
            0xA000..=0xBFFF => panic!("Cartridge RAM not implemented"),
            0xC000..=0xDFFF => self.ram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => self.ram[(addr - 0xE000) as usize], // Echo RAM
            0xFE00..=0xFE9F => self.ppu.read_byte(addr),
            0xFEA0..=0xFEFF => 0, // Unusable memory
            0xFF00..=0xFF7F => self.read_io(addr),
            0xFF80..=0xFFFE => self.zero_page[(addr - 0xFF80) as usize],
            0xFFFF => self.interrupt_controller.read_byte(0xFFFF),
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x7FFF => (), // ROM, read-only
            0x8000..=0x9FFF => self.ppu.write_byte(addr, value),
            0xA000..=0xBFFF => panic!("Cartridge RAM not implemented"),
            0xC000..=0xDFFF => self.ram[(addr - 0xC000) as usize] = value,
            0xE000..=0xFDFF => self.ram[(addr - 0xE000) as usize] = value, // Echo RAM
            0xFE00..=0xFE9F => self.ppu.write_byte(addr, value),
            0xFEA0..=0xFEFF => (), // Unusable memory
            0xFF00..=0xFF7F => self.write_io(addr, value),
            0xFF80..=0xFFFE => self.zero_page[(addr - 0xFF80) as usize] = value,
            0xFFFF => self.interrupt_controller.write_byte(0xFFFF, value),
        }
    }

    pub fn read_word(&self, addr: u16) -> u16 {
        let low = self.read_byte(addr) as u16;
        let high = self.read_byte(addr + 1) as u16;
        (high << 8) | low
    }

    pub fn write_word(&mut self, addr: u16, value: u16) {
        let low = (value & 0xFF) as u8;
        let high = (value >> 8) as u8;
        self.write_byte(addr, low);
        self.write_byte(addr + 1, high);
    }

    fn read_io(&self, addr: u16) -> u8 {
        match addr {
            0xFF00 => panic!("Joypad not implemented"),
            0xFF01..=0xFF02 => panic!("Serial transfer not implemented"),
            0xFF04..=0xFF07 => panic!("Timer not implemented"),
            0xFF0F => self.interrupt_controller.read_byte(addr),
            0xFF10..=0xFF3F => panic!("Audio not implemented"),
            0xFF40..=0xFF4B => self.ppu.read_byte(addr),
            0xFF4C..=0xFF7F => 0, // Unused I/O
            _ => panic!("Unhandled I/O read at address: {:04X}", addr),
        }
    }

    fn write_io(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF00 => panic!("Joypad not implemented"),
            0xFF01..=0xFF02 => panic!("Serial transfer not implemented"),
            0xFF04..=0xFF07 => panic!("Timer not implemented"),
            0xFF0F => self.interrupt_controller.write_byte(addr, value),
            0xFF10..=0xFF3F => panic!("Audio not implemented"),
            0xFF40..=0xFF4B => self.ppu.write_byte(addr, value),
            0xFF50 => self.in_boot = false, // Disable boot ROM
            0xFF4C..=0xFF7F => (), // Unused I/O
            _ => panic!("Unhandled I/O write at address: {:04X}", addr),
        }
    }

    pub fn tick(&mut self, cycles: u32) {
        self.ppu.update(cycles);
        // Update other components here (timer, audio, etc.)
    }
}