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
        loop {
            let cycles = self.cpu.step(&mut self.memory);
            
            // Update components
            self.timer.update(cycles);
            self.ppu.update(cycles);
            
            // Handle interrupts
            let interrupts = self.interrupt_controller.get_interrupts();
            if interrupts != 0 {
                self.cpu.handle_interrupts(&mut self.memory, interrupts);
            }

            // Break condition (you might want to implement a proper exit condition)
            if self.cpu.is_stopped() {
                break;
            }
        }
    }
}

fn main() {
    let mut gameboy = Gameboy::new();
    gameboy.run();
    println!("Emulation finished");
}