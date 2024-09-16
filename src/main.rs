mod cpu;
mod ppu;
mod memory;
mod timer;
mod interrupts;

use cpu::CPU;
use memory::MMU;
use ppu::PPU;
use timer::Timer;
use interrupts::InterruptController;

struct Gameboy {
    cpu: CPU,
    memory: MMU,
    ppu: PPU,
    timer: Timer,
    interrupt_controller: InterruptController,
}

impl Gameboy {
    fn new() -> Self {
        Gameboy {
            cpu: CPU::new(),
            memory: MMU::new(),
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