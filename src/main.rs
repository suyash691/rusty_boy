mod cpu;
mod memory;
mod ppu;
mod timer;
mod interrupts;

use cpu::CPU;
use memory::Memory;
use ppu::PPU;
use timer::Timer;
use interrupts::InterruptController;

struct Gameboy {
    cpu: CPU,
    memory: Memory,
    ppu: PPU,
    timer: Timer,
    interrupt_controller: InterruptController,
}

impl Gameboy {
    fn new() -> Self {
        Gameboy {
            cpu: CPU::new(),
            memory: Memory::new(),
            ppu: PPU::new(),
            timer: Timer::new(),
            interrupt_controller: InterruptController::new(),
        }
    }

    fn run(&mut self) {
    }
}

fn main() {
    let mut gameboy = Gameboy::new();
    gameboy.run();
}