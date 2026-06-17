//! A small, opt-in debugger. The emulator core does not know about tests or
//! debugging — a `Debugger` is configured explicitly by a frontend (the test
//! harness or the CLI) and is empty by default, so real games are unaffected.
//!
//! Breakpoint kinds:
//! - opcode: fires when a given opcode is *executed* (e.g. `0x40` = `LD B,B`,
//!   `0xED` = undefined — the two exit markers used by GB test suites).
//! - pc: fires when the CPU is about to execute the instruction at an address.
//! - memory watch: fires on a read and/or write to an address (checked on the
//!   bus, inside the MMU).

/// Why execution stopped at a breakpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakReason {
    Opcode(u8),
    Pc(u16),
    MemRead(u16),
    MemWrite(u16),
}

pub struct Debugger {
    pub enabled: bool,
    break_opcodes: [bool; 256],
    break_pcs: Vec<u16>,
    watch_reads: Vec<u16>,
    watch_writes: Vec<u16>,
    /// Set by whichever subsystem detects a match; the run loop polls and clears it.
    hit: Option<BreakReason>,
}

impl Default for Debugger {
    fn default() -> Self {
        Debugger {
            enabled: false,
            break_opcodes: [false; 256],
            break_pcs: Vec::new(),
            watch_reads: Vec::new(),
            watch_writes: Vec::new(),
            hit: None,
        }
    }
}

impl Debugger {
    pub fn new() -> Self {
        Debugger::default()
    }

    /// True if any breakpoint is configured (lets hot paths skip all checks).
    fn active(&self) -> bool {
        self.enabled
    }

    // --- configuration (called by frontends) ---

    pub fn add_opcode(&mut self, opcode: u8) {
        self.break_opcodes[opcode as usize] = true;
        self.enabled = true;
    }

    pub fn add_pc(&mut self, pc: u16) {
        self.break_pcs.push(pc);
        self.enabled = true;
    }

    pub fn add_watch_read(&mut self, addr: u16) {
        self.watch_reads.push(addr);
        self.enabled = true;
    }

    pub fn add_watch_write(&mut self, addr: u16) {
        self.watch_writes.push(addr);
        self.enabled = true;
    }

    /// Convenience: register the standard GB test-suite exit markers
    /// (`LD B,B` and the legacy undefined `0xED`).
    pub fn add_test_exit_markers(&mut self) {
        self.add_opcode(0x40);
        self.add_opcode(0xED);
    }

    // --- detection (called by CPU / MMU on the relevant events) ---

    /// Check an opcode about to execute. Cheap no-op when inactive.
    #[inline]
    pub fn check_opcode(&mut self, opcode: u8) {
        if self.active() && self.break_opcodes[opcode as usize] && self.hit.is_none() {
            self.hit = Some(BreakReason::Opcode(opcode));
        }
    }

    /// Check the PC about to be executed. Cheap no-op when inactive.
    #[inline]
    pub fn check_pc(&mut self, pc: u16) {
        if self.active() && self.hit.is_none() && self.break_pcs.contains(&pc) {
            self.hit = Some(BreakReason::Pc(pc));
        }
    }

    #[inline]
    pub fn check_mem_read(&mut self, addr: u16) {
        if self.active() && self.hit.is_none() && self.watch_reads.contains(&addr) {
            self.hit = Some(BreakReason::MemRead(addr));
        }
    }

    #[inline]
    pub fn check_mem_write(&mut self, addr: u16) {
        if self.active() && self.hit.is_none() && self.watch_writes.contains(&addr) {
            self.hit = Some(BreakReason::MemWrite(addr));
        }
    }

    // --- polling (called by the run loop) ---

    /// The pending breakpoint, if any.
    pub fn hit(&self) -> Option<BreakReason> {
        self.hit
    }

    /// Take and clear the pending breakpoint.
    pub fn take_hit(&mut self) -> Option<BreakReason> {
        self.hit.take()
    }
}
