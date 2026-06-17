use minifb::{Key, Window, WindowOptions};
use std::env;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use rusty_boy::cpu::CPU;
use rusty_boy::memory::MMU;

const WIDTH: usize = 160;
const HEIGHT: usize = 144;
const SCALE: usize = 3;
const CYCLES_PER_FRAME: u32 = 70224;

struct Gameboy {
    cpu: CPU,
    mmu: MMU,
    cycle_count: u32,
}

impl Gameboy {
    fn new(rom_path: &str) -> Result<Self, std::io::Error> {
        let mut mmu = MMU::new();
        mmu.load(rom_path)?;
        // Boot into the exact same post-boot hardware state the test suite validates.
        let cpu = rusty_boy::apply_post_boot_state(&mut mmu);
        Ok(Gameboy { cpu, mmu, cycle_count: 0 })
    }

    fn run_frame(&mut self) {
        // The DIV counter advances one tick per T-cycle at the full CPU rate
        // regardless of speed, so its delta measures T-cycles. A PPU frame is
        // CYCLES_PER_FRAME dots; in CGB double-speed the CPU/timer run twice as
        // fast relative to the PPU, so a frame spans twice as many T-cycles.
        let target = CYCLES_PER_FRAME * if self.mmu.double_speed { 2 } else { 1 };
        self.cycle_count = 0;
        while self.cycle_count < target {
            self.cpu.handle_interrupts(&mut self.mmu);
            let before = self.mmu.timer.div_counter();
            self.cpu.step(&mut self.mmu);
            let after = self.mmu.timer.div_counter();
            self.cycle_count += after.wrapping_sub(before) as u32;
        }
        // RTC advances ~1s per 60 frames (no-op for non-MBC3 cartridges).
        self.mmu.tick_rtc();
    }
}

/// Shared audio ring buffer filled by the emulator each frame and consumed by
/// the cpal output callback. Bounded so it can never grow without limit.
type AudioSink = Arc<Mutex<std::collections::VecDeque<f32>>>;

const AUDIO_MAX_SAMPLES: usize = 8192; // ~46ms stereo @ 44.1kHz; drop if the sink falls behind

/// Try to start a cpal output stream. Returns the stream (kept alive) and the
/// shared sink. On any failure (no device, unsupported config) returns None and
/// the emulator runs silently — the per-frame drain still prevents memory growth.
fn start_audio() -> Option<(cpal::Stream, AudioSink)> {
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    let config = device.default_output_config().ok()?;
    let channels = config.channels() as usize;
    let sink: AudioSink = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    let cb_sink = Arc::clone(&sink);

    let stream = device
        .build_output_stream(
            config.into(),
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buf = cb_sink.lock().unwrap();
                for frame in out.chunks_mut(channels) {
                    // Emulator produces interleaved stereo; map L/R onto the
                    // device's channel count, silence if the buffer underruns.
                    let l = buf.pop_front().unwrap_or(0.0);
                    let r = buf.pop_front().unwrap_or(l);
                    for (i, s) in frame.iter_mut().enumerate() {
                        *s = if i % 2 == 0 { l } else { r };
                    }
                }
            },
            |err| eprintln!("audio stream error: {err}"),
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some((stream, sink))
}

/// Parse a number that may be hex (0x..) or decimal.
fn parse_num(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_rom> [--break-opcode N] [--break-pc N] \
                   [--watch-read N] [--watch-write N] [--test-markers]", args[0]);
        eprintln!("  N may be hex (0x40) or decimal. Flags may repeat.");
        std::process::exit(1);
    }

    let mut gameboy = Gameboy::new(&args[1])?;

    // Configure the debugger from CLI flags. Empty by default (real games run
    // untouched); these let you drive breakpoints for manual test/debug runs.
    let mut i = 2;
    let dbg = &mut gameboy.mmu.debugger;
    while i < args.len() {
        // Flags taking a value consume the next arg (hex or decimal).
        let next_val = || args.get(i + 1).and_then(|s| parse_num(s));
        match args[i].as_str() {
            "--break-opcode" => { if let Some(n) = next_val() { dbg.add_opcode(n as u8); } i += 1; }
            "--break-pc" => { if let Some(n) = next_val() { dbg.add_pc(n as u16); } i += 1; }
            "--watch-read" => { if let Some(n) = next_val() { dbg.add_watch_read(n as u16); } i += 1; }
            "--watch-write" => { if let Some(n) = next_val() { dbg.add_watch_write(n as u16); } i += 1; }
            "--test-markers" => dbg.add_test_exit_markers(),
            other => eprintln!("Ignoring unknown argument: {other}"),
        }
        i += 1;
    }

    let mut window = Window::new(
        "Rusty Boy",
        WIDTH * SCALE,
        HEIGHT * SCALE,
        WindowOptions::default(),
    )?;

    window.limit_update_rate(Some(Duration::from_micros(16600)));

    // Audio is best-effort: if no device is available we run silently.
    let audio = start_audio();
    if audio.is_none() {
        eprintln!("No audio output available; running silently.");
    }

    while window.is_open() && !window.is_key_down(Key::Escape) {
        update_input(&window, &mut gameboy.mmu);
        gameboy.run_frame();

        // Report (once) when a debugger breakpoint has fired.
        if let Some(reason) = gameboy.mmu.debugger.take_hit() {
            let c = &gameboy.cpu;
            eprintln!(
                "BREAK {:?} @ PC={:04X}  A={:02X} BC={:02X}{:02X} DE={:02X}{:02X} HL={:02X}{:02X}",
                reason, c.get_pc(), c.get_a(), c.get_b(), c.get_c(),
                c.get_d(), c.get_e(), c.get_h(), c.get_l(),
            );
        }

        // Drain generated samples into the audio sink (bounded), or just clear
        // them if there's no sink, so the APU buffer never grows unbounded.
        let samples = gameboy.mmu.apu.take_samples();
        if let Some((_, sink)) = &audio {
            let mut buf = sink.lock().unwrap();
            if buf.len() < AUDIO_MAX_SAMPLES {
                buf.extend(samples);
            }
        }

        let buffer = gameboy.mmu.get_frame_buffer();
        window.update_with_buffer(&buffer, WIDTH, HEIGHT)?;
    }

    Ok(())
}

/// Map the host keyboard to the Game Boy joypad (active-low nibbles).
/// Buttons nibble: bit0=A bit1=B bit2=Select bit3=Start.
/// D-pad nibble:   bit0=Right bit1=Left bit2=Up bit3=Down.
fn update_input(window: &Window, mmu: &mut MMU) {
    let down = |k: Key| window.is_key_down(k);
    let mut buttons = 0x0F;
    let mut dpad = 0x0F;
    // A press clears the bit (active-low).
    if down(Key::X) { buttons &= !0x01; } // A
    if down(Key::Z) { buttons &= !0x02; } // B
    if down(Key::Backspace) { buttons &= !0x04; } // Select
    if down(Key::Enter) { buttons &= !0x08; } // Start
    if down(Key::Right) { dpad &= !0x01; }
    if down(Key::Left) { dpad &= !0x02; }
    if down(Key::Up) { dpad &= !0x04; }
    if down(Key::Down) { dpad &= !0x08; }
    mmu.joypad.set_state(buttons, dpad);
}
