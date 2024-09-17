use pixels::{Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::KeyCode;
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use std::env;

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

const WIDTH: u32 = 160;
const HEIGHT: u32 = 144;
const SCALE: u32 = 3;

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

        // Update components
        self.timer.update(cycles);
        self.ppu.update(cycles);

        // Handle interrupts
        let interrupts = self.interrupt_controller.get_interrupts();
        if interrupts != 0 {
            self.cpu.handle_interrupts(&mut self.memory, interrupts);
        }
    }

    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = i % WIDTH as usize;
            let y = i / WIDTH as usize;
            let color = self.ppu.framebuffer[y * WIDTH as usize + x];

            let rgba = match color {
                0 => [255, 255, 255, 255], // White
                1 => [192, 192, 192, 255], // Light gray
                2 => [96, 96, 96, 255],    // Dark gray
                3 => [0, 0, 0, 255],       // Black
                _ => unreachable!(),
            };

            pixel.copy_from_slice(&rgba);
        }
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

    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = WindowBuilder::new()
        .with_title("Rusty Boy")
        .with_inner_size(LogicalSize::new(WIDTH * SCALE, HEIGHT * SCALE))
        .build(&event_loop)?;

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture)?
    };

    event_loop.run(move |event, _, control_flow| {
        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            gameboy.draw(pixels.get_frame());
            if pixels.render().is_err() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(KeyCode::Escape) || input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
            }

            // Update internal state and request a redraw
            gameboy.update();
            window.request_redraw();
        }
    });
}