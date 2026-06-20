mod alu;
mod cb_ops;
mod opcodes;

use crate::memory::MMU;

pub struct CPU {
    pub(crate) a: u8,
    pub(crate) b: u8,
    pub(crate) c: u8,
    pub(crate) d: u8,
    pub(crate) e: u8,
    pub(crate) f: u8,
    pub(crate) h: u8,
    pub(crate) l: u8,
    pub(crate) sp: u16,
    pub(crate) pc: u16,
    pub(crate) ime: bool,
    pub(crate) ime_pending: bool, // EI delay: enable IME after next instruction
    pub(crate) halt: bool,
    pub(crate) halt_bug: bool, // HALT bug: next fetch doesn't advance PC
    pub(crate) stop: bool,
}

pub(crate) const ZERO_FLAG: u8 = 0x80;
pub(crate) const SUBTRACT_FLAG: u8 = 0x40;
pub(crate) const HALF_CARRY_FLAG: u8 = 0x20;
pub(crate) const CARRY_FLAG: u8 = 0x10;

impl CPU {
    pub fn new() -> Self {
        CPU {
            a: 0, b: 0, c: 0, d: 0, e: 0, f: 0, h: 0, l: 0,
            sp: 0, pc: 0, ime: false, ime_pending: false,
            halt: false, halt_bug: false, stop: false,
        }
    }

    pub fn get_pc(&self) -> u16 { self.pc }
    pub fn is_halted(&self) -> bool { self.halt }
    pub fn get_a(&self) -> u8 { self.a }
    pub fn get_b(&self) -> u8 { self.b }
    pub fn get_c(&self) -> u8 { self.c }
    pub fn get_d(&self) -> u8 { self.d }
    pub fn get_e(&self) -> u8 { self.e }
    pub fn get_h(&self) -> u8 { self.h }
    pub fn get_l(&self) -> u8 { self.l }

    /// Initialize CPU to post-boot ROM state (DMG)
    pub fn new_post_boot() -> Self {
        CPU {
            a: 0x01, f: 0xB0, b: 0x00, c: 0x13,
            d: 0x00, e: 0xD8, h: 0x01, l: 0x4D,
            sp: 0xFFFE, pc: 0x0100,
            ime: true, ime_pending: false,
            halt: false, halt_bug: false, stop: false,
        }
    }

    /// Initialize CPU to post-boot ROM state (CGB)
    pub fn new_post_boot_cgb() -> Self {
        CPU {
            a: 0x11, f: 0x80, b: 0x00, c: 0x00,
            d: 0xFF, e: 0x56, h: 0x00, l: 0x0D,
            sp: 0xFFFE, pc: 0x0100,
            ime: true, ime_pending: false,
            halt: false, halt_bug: false, stop: false,
        }
    }

    /// Execute one instruction. Returns T-cycles consumed.
    /// All memory accesses inside tick the MMU (cycle-accurate).
    pub fn step(&mut self, memory: &mut MMU) -> u32 {
        // Handle HALT
        if self.halt {
            memory.tick(4);
            return 4;
        }

        // Handle pending EI (enable IME after this instruction)
        let was_ime_pending = self.ime_pending;
        if was_ime_pending {
            self.ime_pending = false;
        }

        // Debugger: PC breakpoint on the instruction about to run.
        memory.debugger.check_pc(self.pc);

        let opcode = self.fetch(memory);
        // Debugger: opcode breakpoint (e.g. test-suite exit markers 0x40 / 0xED).
        memory.debugger.check_opcode(opcode);
        self.execute(opcode, memory);

        // Apply EI delay: IME becomes true after the instruction following EI
        // DI cancels any pending EI effect
        if was_ime_pending && opcode != 0xF3 {
            self.ime = true;
        }

        0 // cycles tracked via tick() calls inside fetch/read/write
    }

    pub fn handle_interrupts(&mut self, memory: &mut MMU) -> u32 {
        let pending = memory.interrupts.pending();
        if pending == 0 { return 0; }

        let was_halted = self.halt;
        self.halt = false;
        if !self.ime { return 0; }

        // HALT-exit timing (CD vs GH split, GateBoy.cpp:451-456 vs :476). A halted CPU un-halts on
        // the DELTA_CD IF sample, half an M-cycle before the DELTA_GH dispatch our `pending()`
        // models. When the ONLY actionable interrupt is STAT and its raise fell in the CD..GH
        // window (the GH grid back-dated it ≥4 phases), the un-halt is one M-cycle later than
        // `pending()` says: withhold the wake this iteration (stay halted, no tick) so `step()`
        // runs its normal idle `tick(4)` — the genuine extra HALT cycle — then dispatch next
        // iteration. One-shot via `stat_halt_deferred` (the phase difference is constant per raise).
        // Scoped to STAT only: non-STAT sources have no back-dating grid and dispatch correctly.
        if was_halted
            && pending & !crate::interrupts::LCD_STAT_INTERRUPT == 0
            && memory.interrupts.stat_halt_should_defer()
        {
            memory.interrupts.stat_halt_deferred = true;
            self.halt = true; // stay halted one more M-cycle (step() runs the idle tick)
            return 0;
        }

        self.ime = false;

        // 2 M-cycles internal delay
        memory.tick(4);
        memory.tick(4);

        for i in 0..5u8 {
            if pending & (1 << i) != 0 {
                memory.interrupts.acknowledge(1 << i);
                // Push high byte of PC
                self.sp = self.sp.wrapping_sub(1);
                log::trace!("INT PUSH HIGH: SP={:04X} val={:02X}", self.sp, (self.pc >> 8) as u8);
                memory.cycle_write(self.sp, (self.pc >> 8) as u8);
                // Hardware: vector is determined by IE state AFTER high byte push
                let still_pending = memory.interrupts.ie & (1 << i);
                // Push low byte of PC
                self.sp = self.sp.wrapping_sub(1);
                log::trace!("INT PUSH LOW: SP={:04X} val={:02X}", self.sp, (self.pc & 0xFF) as u8);
                memory.cycle_write(self.sp, (self.pc & 0xFF) as u8);
                log::trace!("INT CHECK: ie={:02X} bit={} still_pending={}", memory.interrupts.ie, i, still_pending != 0);
                if still_pending != 0 {
                    self.pc = 0x0040 + (i as u16) * 8;
                } else {
                    self.pc = 0x0000;
                }
                memory.tick(4); // Final M-cycle
                return 20;
            }
        }
        0
    }

    // --- Memory access (each one ticks 4 T-cycles) ---

    pub(crate) fn fetch(&mut self, memory: &mut MMU) -> u8 {
        let v = memory.cycle_read(self.pc);
        if self.halt_bug {
            // HALT bug: don't increment PC for this fetch
            self.halt_bug = false;
        } else {
            self.pc = self.pc.wrapping_add(1);
        }
        v
    }

    pub(crate) fn fetch_word(&mut self, memory: &mut MMU) -> u16 {
        let lo = self.fetch(memory);
        let hi = self.fetch(memory);
        u16::from_le_bytes([lo, hi])
    }

    pub(crate) fn read_byte(&mut self, memory: &mut MMU, addr: u16) -> u8 {
        memory.cycle_read(addr)
    }

    pub(crate) fn write_byte(&mut self, memory: &mut MMU, addr: u16, value: u8) {
        memory.cycle_write(addr, value);
    }

    /// Internal tick (no memory access, just time passing)
    pub(crate) fn internal_cycle(&mut self, memory: &mut MMU) {
        memory.tick(4);
    }

    // --- Register pair helpers ---
    pub(crate) fn get_bc(&self) -> u16 { u16::from_le_bytes([self.c, self.b]) }
    pub(crate) fn set_bc(&mut self, v: u16) { let [c, b] = v.to_le_bytes(); self.b = b; self.c = c; }
    pub(crate) fn get_de(&self) -> u16 { u16::from_le_bytes([self.e, self.d]) }
    pub(crate) fn set_de(&mut self, v: u16) { let [e, d] = v.to_le_bytes(); self.d = d; self.e = e; }
    pub(crate) fn get_hl(&self) -> u16 { u16::from_le_bytes([self.l, self.h]) }
    pub(crate) fn set_hl(&mut self, v: u16) { let [l, h] = v.to_le_bytes(); self.h = h; self.l = l; }
    pub(crate) fn get_af(&self) -> u16 { u16::from_le_bytes([self.f, self.a]) }
    pub(crate) fn set_af(&mut self, v: u16) { let [f, a] = v.to_le_bytes(); self.a = a; self.f = f & 0xF0; }

    // --- Flag helpers ---
    pub(crate) fn set_flag(&mut self, flag: u8) { self.f |= flag; }
    pub(crate) fn clear_flag(&mut self, flag: u8) { self.f &= !flag; }
    pub(crate) fn is_flag_set(&self, flag: u8) -> bool { self.f & flag != 0 }

    // --- Stack (cycle-accurate) ---
    pub(crate) fn push_stack(&mut self, memory: &mut MMU, value: u16) {
        self.sp = self.sp.wrapping_sub(1);
        self.write_byte(memory, self.sp, (value >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.write_byte(memory, self.sp, (value & 0xFF) as u8);
    }

    pub(crate) fn pop_stack(&mut self, memory: &mut MMU) -> u16 {
        let lo = self.read_byte(memory, self.sp);
        self.sp = self.sp.wrapping_add(1);
        let hi = self.read_byte(memory, self.sp);
        self.sp = self.sp.wrapping_add(1);
        u16::from_le_bytes([lo, hi])
    }

    // --- CB register access ---
    pub(crate) fn get_r(&mut self, r: u8, memory: &mut MMU) -> u8 {
        match r {
            0 => self.b, 1 => self.c, 2 => self.d, 3 => self.e,
            4 => self.h, 5 => self.l,
            6 => self.read_byte(memory, self.get_hl()),
            7 => self.a,
            _ => unreachable!(),
        }
    }

    pub(crate) fn set_r(&mut self, r: u8, value: u8, memory: &mut MMU) {
        match r {
            0 => self.b = value, 1 => self.c = value, 2 => self.d = value, 3 => self.e = value,
            4 => self.h = value, 5 => self.l = value,
            6 => { let addr = self.get_hl(); self.write_byte(memory, addr, value); }
            7 => self.a = value,
            _ => unreachable!(),
        }
    }
}
