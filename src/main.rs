use minifb::{Key, Window, WindowOptions};
use std::env;
use std::time::Duration;

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

const WIDTH: usize = 160;
const HEIGHT: usize = 144;
const SCALE: usize = 3;

struct Gameboy {
    cpu: CPU,
    memory: MMU,
    ppu: PPU,
    timer: Timer,
    interrupt_controller: InterruptController,
}

impl Gameboy {
    fn new(rom_path: &str) -> Result<Self, std::io::Error> {
        let mut memory = MMU::new();
        memory.load(rom_path)?;
        println!("ROM loaded successfully");

        Ok(Gameboy {
            cpu: CPU::new(),
            memory,
            ppu: PPU::new(),
            timer: Timer::new(),
            interrupt_controller: InterruptController::new(),
        })
    }

    fn update(&mut self) {
        let cycles = self.cpu.step(&mut self.memory);
        println!("CPU executed {} cycles", cycles);

        self.timer.update(cycles);
        self.ppu.update(cycles);

        let interrupts = self.interrupt_controller.get_interrupts();
        if interrupts != 0 {
            self.cpu.handle_interrupts(&mut self.memory, interrupts);
        }
    }

    fn get_frame_buffer(&self) -> Vec<u32> {
        self.ppu.get_frame_buffer()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path_to_rom>", args[0]);
        std::process::exit(1);
    }

    let rom_path = &args[1];
    let mut gameboy = Gameboy::new(rom_path)?;
    println!("First ROM byte: 0x{:02X}", gameboy.memory.read_byte(0x0100));
    println!("Nintendo logo byte: 0x{:02X}", gameboy.memory.read_byte(0x0104));

    let mut window = Window::new(
        "Rusty Boy",
        WIDTH * SCALE,
        HEIGHT * SCALE,
        WindowOptions::default(),
    )?;

    window.limit_update_rate(Some(Duration::from_micros(16600))); // ~60 fps

    let mut frame_count = 0;
    while window.is_open() && !window.is_key_down(Key::Escape) {
        gameboy.update();

        let buffer: Vec<u32> = gameboy.get_frame_buffer();
        window.update_with_buffer(&buffer, WIDTH, HEIGHT)?;

        frame_count += 1;
        if frame_count % 60 == 0 {
            println!("Rendered 60 frames");
        }
    }

    Ok(())
}