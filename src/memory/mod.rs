mod hdma;
mod mbc;

use std::fs::File;
use std::io::Read;

use crate::apu::APU;
use crate::debugger::Debugger;
use crate::interrupts::{InterruptController, TIMER_INTERRUPT, VBLANK_INTERRUPT};
use crate::ppu::PPU;
use crate::timer::Timer;

pub struct MMU {
    boot_rom: [u8; 256],
    pub(crate) rom: Vec<u8>,
    pub(crate) wram_bank: [u8; 0x8000],
    hram: [u8; 127],
    in_boot: bool,
    pub ppu: PPU,
    pub timer: Timer,
    pub interrupts: InterruptController,
    pub joypad: Joypad,
    pub apu: APU,
    pub(crate) mbc_type: u8,
    pub(crate) rom_bank: u16,
    pub(crate) ram_bank: u8,
    pub(crate) ram_enabled: bool,
    pub(crate) mbc_mode: u8,
    pub(crate) cart_ram: Vec<u8>,
    pub serial_data: u8,
    pub serial_control: u8,
    pub serial_output: Vec<u8>,
    pub cgb_mode: bool,
    pub double_speed: bool,
    pub(crate) speed_switch_armed: bool,
    pub(crate) wram_bank_num: u8,
    pub(crate) hdma_src: u16,
    pub(crate) hdma_dst: u16,
    pub(crate) hdma_len: u8,
    pub(crate) hdma_active: bool,
    pub(crate) hdma_hblank: bool,
    // RTC (MBC3)
    pub(crate) rtc_regs: [u8; 5],
    pub(crate) rtc_latched: [u8; 5],
    pub(crate) rtc_latch_prev: u8,
    pub(crate) rtc_sub_seconds: u8,
    pub debugger: Debugger,
}

pub struct Joypad {
    pub select: u8,
    pub buttons: u8,
    pub dpad: u8,
    pub interrupt_requested: bool,
}

impl Joypad {
    pub fn new() -> Self { Joypad { select: 0x30, buttons: 0x0F, dpad: 0x0F, interrupt_requested: false } }
    pub fn read(&self) -> u8 {
        let mut r = self.select | 0xC0;
        if self.select & 0x10 == 0 { r |= self.dpad; }
        if self.select & 0x20 == 0 { r |= self.buttons; }
        r
    }
    pub fn write(&mut self, value: u8) { self.select = value & 0x30; }

    /// Update button/dpad state from the frontend. Nibbles are active-low
    /// (bit clear = pressed). Raises the joypad interrupt on any press edge
    /// (a line going high→low), which `collect_interrupts` delivers to IF.
    pub fn set_state(&mut self, buttons: u8, dpad: u8) {
        let pressed_btn = self.buttons & !buttons; // bits newly cleared
        let pressed_dpad = self.dpad & !dpad;
        if pressed_btn != 0 || pressed_dpad != 0 {
            self.interrupt_requested = true;
        }
        self.buttons = buttons & 0x0F;
        self.dpad = dpad & 0x0F;
    }
}

impl MMU {
    pub fn new() -> Self {
        MMU {
            boot_rom: [0; 256], rom: Vec::new(), wram_bank: [0; 0x8000], hram: [0; 127],
            in_boot: false, ppu: PPU::new(), timer: Timer::new(),
            interrupts: InterruptController::new(), joypad: Joypad::new(), apu: APU::new(),
            mbc_type: 0, rom_bank: 1, ram_bank: 0, ram_enabled: false, mbc_mode: 0,
            cart_ram: vec![0; 0x8000], serial_data: 0, serial_control: 0, serial_output: Vec::new(),
            cgb_mode: false, double_speed: false, speed_switch_armed: false, wram_bank_num: 1,
            hdma_src: 0, hdma_dst: 0, hdma_len: 0xFF, hdma_active: false, hdma_hblank: false,
            rtc_regs: [0; 5], rtc_latched: [0; 5], rtc_latch_prev: 0, rtc_sub_seconds: 0,
            debugger: Debugger::new(),
        }
    }

    pub fn load(&mut self, filename: &str) -> std::io::Result<()> {
        let mut file = File::open(filename)?;
        self.rom.clear();
        file.read_to_end(&mut self.rom)?;
        if self.rom.len() > 0x0147 { self.mbc_type = self.rom[0x0147]; }
        if self.rom.len() > 0x0143 { self.cgb_mode = self.rom[0x0143] & 0x80 != 0; }
        Ok(())
    }

    pub fn load_boot_rom(&mut self, filename: &str) -> std::io::Result<()> {
        let mut file = File::open(filename)?;
        file.read_exact(&mut self.boot_rom)?;
        self.in_boot = true;
        Ok(())
    }

    /// Generic time advance (internal CPU cycles with no bus access).
    /// Always called with one M-cycle (4 T-cycles). The timer runs at the full
    /// T-cycle rate regardless of speed; only PPU/APU dots scale in double-speed.
    pub fn tick(&mut self, t_cycles: u32) {
        self.timer.update(t_cycles);
        let div_bit = self.timer.div_counter() & (1 << 12) != 0;
        self.tick_dots(t_cycles, div_bit);
        self.settle_m_cycle();
    }

    /// Advance the dot-clock subsystems (PPU + APU) by `t_cycles` of time, applying
    /// double-speed dot scaling. Does NOT tick the timer, collect interrupts, or
    /// advance DMA — the caller owns those so the sub-M-cycle ordering stays exact.
    fn tick_dots(&mut self, t_cycles: u32, div_bit: bool) {
        let dots = if self.double_speed { t_cycles / 2 } else { t_cycles };
        // The PPU now advances per PHASE (2 phases = 1 dot), so emit 2 phases per dot.
        for _ in 0..dots { self.ppu.update(2); }
        self.apu.update_with_div(dots, div_bit);
    }

    /// End-of-M-cycle settle: move subsystem interrupts into IF, advance OAM DMA,
    /// and service HBlank HDMA on the rising edge of HBlank entry.
    fn settle_m_cycle(&mut self) {
        self.collect_interrupts();
        self.advance_dma();
        if self.ppu.hblank_entered {
            self.ppu.hblank_entered = false;
            self.tick_hdma();
        }
    }

    /// One full bus M-cycle with the load-bearing 3-dots / access / 1-dot split.
    /// `access` performs the actual read or write between the two dot phases.
    /// The APU `div_bit` is sampled once, at the end of the M-cycle.
    fn bus_cycle<R>(&mut self, access: impl FnOnce(&mut Self) -> R) -> R {
        // Phase 1: 3 T-cycles of timer, then the pre-access PPU phases.
        self.timer.update(3);
        // The PPU now ticks per phase (2 phases = 1 dot), so the M-cycle is 8 phases
        // (4 in double-speed). The access samples the PPU lock/mode at the dot-3
        // boundary — 6 phases before / 2 after (double-speed 2 / 2). This is the same
        // 3-dots/access/1-dot split as the old per-dot model. (Sampling at phase 7 — the
        // GateBoy DELTA_HA read-latch phase — was tried and regressed the OAM/VRAM lock
        // tests, so the dot-3 boundary is retained; see zazzy-dreaming-ocean.md.)
        let pre_phases = if self.double_speed { 2 } else { 6 };
        self.ppu.update(pre_phases);

        // Commit pending STAT dispatch/read visibility as of this M-cycle's access phase,
        // so both dispatch and a $FF0F read see the phase-exact latched value.
        self.interrupts.commit_stat_dispatch(self.ppu.phase_lcd);
        self.interrupts.commit_stat_if(self.ppu.phase_lcd);

        // Bus access happens at the dot-3 boundary.
        let r = access(self);

        // Phase 2: final T-cycle of timer + 2 PPU phases (1 dot), then settle.
        self.timer.update(1);
        self.ppu.update(2);
        let div_bit = self.timer.div_counter() & (1 << 12) != 0;
        let apu_cycles = if self.double_speed { 2 } else { 4 };
        self.apu.update_with_div(apu_cycles, div_bit);
        self.settle_m_cycle();
        r
    }

    /// Advance PPU by N dots (1 dot at a time for accuracy)
    /// Collect pending interrupts from subsystems into IF
    fn collect_interrupts(&mut self) {
        if self.ppu.vblank_interrupt { self.ppu.vblank_interrupt = false; self.interrupts.request(VBLANK_INTERRUPT); }
        // STAT is dispatchable now but becomes CPU-readable in IF only at its publish phase
        // (the $FF0F read-latch). Seed pending+publish-phase, then commit any pending STAT
        // whose phase has passed (covers non-read M-cycles; the read path also commits at
        // its access point in bus_cycle).
        if self.ppu.stat_interrupt {
            self.ppu.stat_interrupt = false;
            // DISPATCH-visible at the GH boundary on hardware's grid: bucket =
            // floor((raise_dot - 1)/4), so the IRQ becomes dispatchable at the next grid
            // boundary dot = ((raise_dot-1)/4 + 1)*4 + 1 (1-dot offset from a naive
            // per-M-cycle settle). Measured exact vs gbmicrotest int_hblank_incs/nops/halt.
            let raise = self.ppu.stat_raise_phase;
            let raise_dot = raise / 2;
            // GH dispatch grid bucket = floor((raise_dot-1)/4); dispatch becomes visible at
            // that bucket's M-cycle boundary (dot ≡1 mod 4). Measured vs int_hblank_*.
            let dispatch_dot = ((raise_dot - 1) / 4) * 4 + 1;
            let dispatch_phase = dispatch_dot * 2;
            self.interrupts.request_stat(dispatch_phase, self.ppu.stat_publish_phase);
        }
        self.interrupts.commit_stat_dispatch(self.ppu.phase_lcd);
        self.interrupts.commit_stat_if(self.ppu.phase_lcd);
        if self.timer.interrupt_requested { self.timer.interrupt_requested = false; self.interrupts.request(TIMER_INTERRUPT); }
        if self.joypad.interrupt_requested { self.joypad.interrupt_requested = false; self.interrupts.request(crate::interrupts::JOYPAD_INTERRUPT); }
    }

    fn advance_dma(&mut self) {
        if self.ppu.dma_active {
            if self.ppu.dma_delay > 0 { self.ppu.dma_delay -= 1; }
            else { let src = self.ppu.dma_source + self.ppu.dma_offset as u16; let byte = self.dma_read(src); self.ppu.dma_write_oam(byte); }
        }
    }

    /// Bus read: PPU runs 3 dots, CPU reads, PPU runs 1 dot.
    /// During OAM DMA the CPU sees 0xFF for any access outside HRAM (bus conflict).
    pub fn cycle_read(&mut self, addr: u16) -> u8 {
        self.debugger.check_mem_read(addr);
        self.bus_cycle(|s| {
            if s.ppu.dma_active && !(0xFF00..=0xFFFF).contains(&addr) { 0xFF }
            else { s.read_byte(addr) }
        })
    }

    /// Bus write: PPU runs 3 dots, CPU writes, PPU runs 1 dot.
    pub fn cycle_write(&mut self, addr: u16, value: u8) {
        self.debugger.check_mem_write(addr);
        // pre_write_bit MUST be captured before any timer ticking for accurate
        // TIMA edge detection on timer-register writes.
        if (0xFF04..=0xFF07).contains(&addr) {
            self.timer.pre_write_bit = self.timer.get_timer_bit_pub();
        }
        self.bus_cycle(|s| {
            if !(s.ppu.dma_active && !(0xFF00..=0xFFFF).contains(&addr)) {
                s.write_byte(addr, value);
            }
        });
    }

    fn dma_read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.rom_read(addr),
            0x8000..=0x9FFF => self.ppu.read_vram(addr),
            0xC000..=0xCFFF => self.wram_bank[(addr - 0xC000) as usize],
            0xD000..=0xDFFF => self.wram_bank[self.wram_bank_num as usize * 0x1000 + (addr - 0xD000) as usize],
            _ => 0xFF,
        }
    }

    pub fn get_frame_buffer(&self) -> Vec<u32> { self.ppu.get_frame_buffer() }
    pub fn framebuffer_shades(&self) -> Vec<u8> { self.ppu.framebuffer_shades() }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00FF if self.in_boot => self.boot_rom[addr as usize],
            0x0000..=0x7FFF => self.rom_read(addr),
            // read_vram applies the mode-3 block itself — single source of truth
            // for VRAM visibility. (The PPU renderer reads its vram fields directly
            // and intentionally bypasses this, since it runs *during* mode 3.)
            0x8000..=0x9FFF => self.ppu.read_vram(addr),
            0xA000..=0xBFFF => self.cart_ram_read(addr),
            0xC000..=0xCFFF => self.wram_bank[(addr - 0xC000) as usize],
            0xD000..=0xDFFF => self.wram_bank[self.wram_bank_num as usize * 0x1000 + (addr - 0xD000) as usize],
            0xE000..=0xEFFF => self.wram_bank[(addr - 0xE000) as usize],
            0xF000..=0xFDFF => self.wram_bank[self.wram_bank_num as usize * 0x1000 + (addr - 0xF000) as usize],
            0xFE00..=0xFE9F => self.ppu.read_oam(addr), // read_oam applies the read-lock
            0xFEA0..=0xFEFF => 0xFF,
            0xFF00..=0xFF7F => self.read_io(addr),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.interrupts.read_byte(addr),
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x7FFF => self.mbc_write(addr, value),
            0x8000..=0x9FFF => {
                if !self.ppu.vram_locked(true) { self.ppu.write_vram(addr, value); }
            }
            0xA000..=0xBFFF => self.cart_ram_write(addr, value),
            0xC000..=0xCFFF => self.wram_bank[(addr - 0xC000) as usize] = value,
            0xD000..=0xDFFF => self.wram_bank[self.wram_bank_num as usize * 0x1000 + (addr - 0xD000) as usize] = value,
            0xE000..=0xEFFF => self.wram_bank[(addr - 0xE000) as usize] = value,
            0xF000..=0xFDFF => self.wram_bank[self.wram_bank_num as usize * 0x1000 + (addr - 0xF000) as usize] = value,
            0xFE00..=0xFE9F => {
                if !self.ppu.oam_locked(true) { self.ppu.write_oam(addr, value); }
            }
            0xFEA0..=0xFEFF => {}
            0xFF00..=0xFF7F => self.write_io(addr, value),
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = value,
            0xFFFF => self.interrupts.write_byte(addr, value),
        }
    }

    fn read_io(&self, addr: u16) -> u8 {
        match addr {
            0xFF00 => self.joypad.read(),
            0xFF01 => self.serial_data,
            0xFF02 => self.serial_control | 0x7E,
            0xFF04..=0xFF07 => self.timer.read_byte(addr),
            0xFF0F => self.interrupts.read_byte(addr),
            0xFF10..=0xFF3F => self.apu.read_byte(addr),
            0xFF40..=0xFF4B => self.ppu.read_io(addr),
            0xFF4D => (if self.double_speed { 0x80 } else { 0 }) | (if self.speed_switch_armed { 0x01 } else { 0 }) | 0x7E,
            0xFF4F => self.ppu.read_vbk(),
            0xFF51..=0xFF55 => self.read_hdma(addr),
            0xFF68..=0xFF6B => self.ppu.read_cgb_palette(addr),
            0xFF70 => self.wram_bank_num | 0xF8,
            _ => 0xFF,
        }
    }

    fn write_io(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF00 => self.joypad.write(value),
            0xFF01 => self.serial_data = value,
            0xFF02 => { self.serial_control = value; if value == 0x81 { self.serial_output.push(self.serial_data); self.serial_control &= 0x7F; } }
            0xFF04..=0xFF07 => self.timer.write_byte(addr, value),
            0xFF0F => self.interrupts.write_byte(addr, value),
            0xFF10..=0xFF3F => self.apu.write_byte(addr, value),
            0xFF40..=0xFF4B => self.ppu.write_io(addr, value),
            0xFF4D => self.speed_switch_armed = value & 0x01 != 0,
            0xFF4F => self.ppu.write_vbk(value),
            0xFF50 => self.in_boot = false,
            0xFF51..=0xFF55 => self.write_hdma(addr, value),
            0xFF68..=0xFF6B => self.ppu.write_cgb_palette(addr, value),
            0xFF70 => { self.wram_bank_num = value & 0x07; if self.wram_bank_num == 0 { self.wram_bank_num = 1; } }
            _ => {}
        }
    }
}
