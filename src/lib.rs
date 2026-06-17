pub mod apu;
pub mod cpu;
pub mod debugger;
pub mod interrupts;
pub mod memory;
pub mod ppu;
pub mod timer;
#[cfg(test)]
mod timer_tests;

use cpu::CPU;
use memory::MMU;

/// Create a DMG-state emulator loaded with a ROM.
pub fn init_dmg(rom_path: &str) -> (CPU, MMU) {
    let mut mmu = MMU::new();
    mmu.load(rom_path).expect("Failed to load ROM");
    let cpu = apply_post_boot_state(&mut mmu);
    (cpu, mmu)
}

/// Apply the post-boot hardware register state to an already-loaded MMU and
/// return a CPU initialized to the matching post-boot register values.
/// Shared by `init_dmg` (tests) and the `main` binary so the running machine
/// boots into exactly the same state the test suite validates.
pub fn apply_post_boot_state(mmu: &mut MMU) -> CPU {
    let cpu = if mmu.cgb_mode {
        mmu.ppu.cgb_mode = true;
        CPU::new_post_boot_cgb()
    } else {
        CPU::new_post_boot()
    };
    mmu.write_byte(0xFF05, 0x00);
    mmu.write_byte(0xFF06, 0x00);
    mmu.write_byte(0xFF07, 0x00);
    mmu.write_byte(0xFF40, 0x91);
    mmu.write_byte(0xFF47, 0xFC);
    mmu.write_byte(0xFF48, 0xFF); // OBP0 uninitialized
    mmu.write_byte(0xFF49, 0xFF); // OBP1 uninitialized
    // APU post-boot state (NR52=$F1 = enabled, ch1 active)
    mmu.write_byte(0xFF26, 0x80); // Enable APU first
    mmu.write_byte(0xFF10, 0x80);
    mmu.write_byte(0xFF11, 0xBF);
    mmu.write_byte(0xFF12, 0xF3);
    mmu.write_byte(0xFF14, 0xBF);
    mmu.write_byte(0xFF16, 0x3F);
    mmu.write_byte(0xFF24, 0x77);
    mmu.write_byte(0xFF25, 0xF3);
    // Post-boot IF, SC, P1
    mmu.interrupts.if_ = 0x01; // VBlank flag set after boot (reads as 0xE1 with upper bits OR)
    mmu.serial_control = 0x7E;
    mmu.joypad.select = 0x00; // No button/dpad selection → P1 reads 0xCF
    mmu.timer.set_div(0xABCC);
    // Boot ROM handoff: seed the PPU to the mid-frame boot phase (NOT the software
    // enable-quirk the 0xFF40=0x91 write above triggered). poweron_* anchors here.
    mmu.ppu.boot_init();
    cpu
}

/// Run the emulator until a condition is met or timeout.
/// The emulator doesn't know about tests — it just executes.
pub fn run_until(
    cpu: &mut CPU,
    mmu: &mut MMU,
    timeout: std::time::Duration,
    stop_condition: impl Fn(&MMU, u64) -> bool,
) -> u64 {
    let deadline = std::time::Instant::now() + timeout;
    let mut steps: u64 = 0;
    loop {
        cpu.handle_interrupts(mmu);
        cpu.step(mmu);
        steps += 1;
        if steps % 10000 == 0 {
            if stop_condition(mmu, steps) || std::time::Instant::now() >= deadline {
                return steps;
            }
        }
    }
}

/// Run until a debugger breakpoint fires (e.g. a test-suite `LD B,B` exit
/// marker) or the timeout elapses. Returns the breakpoint reason if one was
/// hit. Unlike `run_until`, the caller can then inspect CPU registers — needed
/// by suites that report results in registers rather than serial/memory.
pub fn run_until_break(
    cpu: &mut CPU,
    mmu: &mut MMU,
    timeout: std::time::Duration,
) -> Option<debugger::BreakReason> {
    let deadline = std::time::Instant::now() + timeout;
    let mut steps: u64 = 0;
    loop {
        cpu.handle_interrupts(mmu);
        cpu.step(mmu);
        if let Some(reason) = mmu.debugger.hit() {
            return Some(reason);
        }
        steps += 1;
        if steps % 10000 == 0 && std::time::Instant::now() >= deadline {
            return None;
        }
    }
}
