//! Unit tests for timer edge cases
#[cfg(test)]
mod tests {
    use crate::timer::Timer;

    /// div_write test scenario:
    /// 1. Enable timer with TAC mode 01 (bit 3, period=16 T-cycles)
    /// 2. Tick until DIV bit 3 is high  
    /// 3. Write to FF04 (reset DIV) - should trigger falling edge → TIMA++
    ///
    /// The key: the write happens BETWEEN T3 and T4 of the write M-cycle.
    /// So timer ticks 3 times, then the write fires, then 1 more tick.
    /// The bit must be HIGH after those 3 ticks for the edge to be detected.
    #[test]
    fn div_write_triggers_tima_increment() {
        let mut timer = Timer::new();
        
        // Enable timer, mode 01 (bit 3, increments every 16 T-cycles)
        timer.write_byte(0xFF07, 0x05); // TAC = enabled | mode 01
        
        // TIMA starts at 0
        assert_eq!(timer.read_byte(0xFF05), 0);
        
        // Tick until DIV bit 3 is HIGH
        // Bit 3 goes high when DIV reaches 8 (0b1000)
        // We need to position so that AFTER 3 more ticks, bit 3 is still high.
        // Bit 3 is high for DIV values 8-15, then low for 0-7, cycling every 16.
        // So tick to DIV=8, then the 3 pre-ticks make DIV=11, bit 3 still high.
        timer.update(8);
        
        println!("After 8 ticks: DIV={}, bit3={}, TIMA={}", 
            timer.div_counter(), timer.div_counter() & 8 != 0, timer.read_byte(0xFF05));
        assert_eq!(timer.div_counter(), 8);
        assert!(timer.div_counter() & 8 != 0, "Bit 3 should be high");
        assert_eq!(timer.read_byte(0xFF05), 0, "TIMA should still be 0");
        
        // Simulate cycle_write(FF04): tick 3 times, then write
        timer.update(3); // T1-T3: DIV goes to 11
        println!("After 3 pre-ticks: DIV={}, bit3={}", timer.div_counter(), timer.div_counter() & 8 != 0);
        assert!(timer.div_counter() & 8 != 0, "Bit 3 should still be high at DIV=11");
        
        // Write fires - DIV resets, falling edge should be detected
        timer.write_byte(0xFF04, 0);
        println!("After DIV write: DIV={}, TIMA={}", timer.div_counter(), timer.read_byte(0xFF05));
        
        timer.update(1); // T4
        println!("After T4: DIV={}, TIMA={}", timer.div_counter(), timer.read_byte(0xFF05));
        
        // TIMA should have incremented from the falling edge
        assert_eq!(timer.read_byte(0xFF05), 1, "TIMA should be 1 after DIV write caused falling edge");
    }

    /// Test that a normal tick through the falling edge also works
    #[test]
    fn normal_falling_edge_increments_tima() {
        let mut timer = Timer::new();
        timer.write_byte(0xFF07, 0x05); // TAC enabled, mode 01 (bit 3)
        
        // Tick 16 times to go through one full bit 3 cycle (0→1→0)
        // The falling edge happens when DIV goes from 15→16 (bit 3: 1→0)
        timer.update(16);
        
        println!("After 16 ticks: DIV={}, TIMA={}", timer.div_counter(), timer.read_byte(0xFF05));
        assert_eq!(timer.read_byte(0xFF05), 1, "TIMA should be 1 after falling edge at DIV=16");
    }

    /// Test that the 3-tick pre-write doesn't consume the edge that the DIV write should get
    #[test]  
    fn div_write_at_falling_edge_boundary() {
        let mut timer = Timer::new();
        timer.write_byte(0xFF07, 0x05); // TAC enabled, mode 01 (bit 3)
        
        // Position DIV at 15 (bit 3 high, about to go low on next tick)
        timer.update(15);
        println!("DIV={}, bit3={}, TIMA={}", timer.div_counter(), timer.div_counter() & 8 != 0, timer.read_byte(0xFF05));
        assert_eq!(timer.div_counter(), 15);
        assert!(timer.div_counter() & 8 != 0);
        assert_eq!(timer.read_byte(0xFF05), 0);
        
        // Simulate cycle_write(FF04): tick 3
        // T1: DIV=16, bit 3 goes 1→0 → NORMAL falling edge fires, TIMA=1
        // T2: DIV=17, bit 3=0
        // T3: DIV=18, bit 3=0
        timer.update(3);
        println!("After 3 ticks: DIV={}, TIMA={}", timer.div_counter(), timer.read_byte(0xFF05));
        // The normal edge fired at DIV=16!
        let tima_before_write = timer.read_byte(0xFF05);
        println!("TIMA before write: {}", tima_before_write);
        
        // Now DIV write fires. Bit 3 is currently LOW (DIV=18).
        // The write handler checks get_timer_bit() which is false.
        // So NO additional increment from the write. Total TIMA=1 (from normal edge only).
        timer.write_byte(0xFF04, 0);
        timer.update(1);
        
        println!("Final TIMA: {}", timer.read_byte(0xFF05));
        // On real hardware, the DIV write at this position should result in TIMA=1
        // (the edge was already consumed by normal tick, and the DIV reset finds bit low)
        assert_eq!(timer.read_byte(0xFF05), 1);
    }
    
    /// Reproduce the exact scenario div_write mooneye test checks:
    /// The test positions DIV so that bit is HIGH at the write point.
    /// After tick(3), the bit should STILL be high.
    #[test]
    fn div_write_bit_still_high_after_3_preticks() {
        let mut timer = Timer::new();
        timer.write_byte(0xFF07, 0x05); // TAC mode 01 (bit 3)
        
        // Position at DIV=9. After 3 ticks → DIV=12. Bit 3 still high (8-15).
        timer.update(9);
        assert_eq!(timer.div_counter(), 9);
        
        // Simulate write M-cycle
        timer.update(3);
        assert_eq!(timer.div_counter(), 12);
        assert!(timer.div_counter() & 8 != 0, "Bit 3 should be high at DIV=12");
        assert_eq!(timer.read_byte(0xFF05), 0, "No edge should have fired yet (bit stayed high)");
        
        // DIV write - bit is high, so falling edge fires
        timer.write_byte(0xFF04, 0);
        assert_eq!(timer.read_byte(0xFF05), 1, "TIMA should increment from DIV reset falling edge");
    }
}

    /// Run the actual div_write ROM and check what happens
    #[test]
    fn trace_div_write_rom() {
        let rom_path = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/timer/div_write.gb");
        if !std::path::Path::new(rom_path).exists() { return; }
        
        use crate::cpu::CPU;
        use crate::memory::MMU;
        
        let mut mmu = MMU::new();
        mmu.load(rom_path).unwrap();
        let mut cpu = CPU::new_post_boot();
        mmu.write_byte(0xFF05, 0x00);
        mmu.write_byte(0xFF06, 0x00);
        mmu.write_byte(0xFF07, 0x00);
        mmu.write_byte(0xFF40, 0x91);
        mmu.write_byte(0xFF47, 0xFC);
        mmu.timer.set_div(0xABCC);
        
        // Run for a limited number of steps and track PC
        let mut last_pcs = Vec::new();
        for i in 0..500000u32 {
            cpu.handle_interrupts(&mut mmu);
            let pc_before = cpu.get_pc();
            cpu.step(&mut mmu);
            
            if i > 499900 {
                last_pcs.push(pc_before);
            }
        }
        
        // Check if stuck in a loop
        let unique_pcs: std::collections::HashSet<u16> = last_pcs.iter().copied().collect();
        println!("Last 100 PCs: {} unique values", unique_pcs.len());
        println!("PCs: {:04X?}", &last_pcs[..last_pcs.len().min(20)]);
        println!("TIMA: {}", mmu.read_byte(0xFF05));
        println!("TAC: {:02X}", mmu.read_byte(0xFF07));
        println!("DIV: {}", mmu.timer.div_counter());
        println!("Serial: {:?}", &mmu.serial_output);
        // Print ROM at loop address
        print!("ROM @0160: ");
        for i in 0x160..0x175u16 { print!("{:02X} ", mmu.read_byte(i)); }
        println!();
        // Print CPU regs
        println!("A={:02X} B={:02X} C={:02X}", cpu.get_a(), cpu.get_b(), cpu.get_c());
        
        // The test should NOT be stuck - it should eventually produce output
        assert!(!mmu.serial_output.is_empty() || unique_pcs.len() > 5,
            "Test appears stuck in a {} PC loop", unique_pcs.len());
    }

    #[test]
    fn trace_rapid_toggle_rom() {
        let rom_path = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/timer/rapid_toggle.gb");
        if !std::path::Path::new(rom_path).exists() { return; }
        use crate::cpu::CPU;
        use crate::memory::MMU;
        let mut mmu = MMU::new();
        mmu.load(rom_path).unwrap();
        let mut cpu = CPU::new_post_boot();
        mmu.write_byte(0xFF05, 0x00); mmu.write_byte(0xFF06, 0x00);
        mmu.write_byte(0xFF07, 0x00); mmu.write_byte(0xFF40, 0x91);
        mmu.write_byte(0xFF47, 0xFC); mmu.timer.set_div(0xABCC);
        
        // Find the toggle loop
        for _ in 0..1000000u32 {
            cpu.handle_interrupts(&mut mmu);
            cpu.step(&mut mmu);
            if !mmu.serial_output.is_empty() { break; }
        }
        println!("Serial: {:?}", &mmu.serial_output);
        println!("TIMA: {}", mmu.read_byte(0xFF05));
        // Dump ROM at common loop points
        print!("ROM @0150: ");
        for i in 0x150..0x170u16 { print!("{:02X} ", mmu.read_byte(i)); }
        println!();
    }

    #[test]
    fn tima_write_during_reload_is_ignored() {
        use crate::timer::Timer;
        let mut timer = Timer::new();
        timer.write_byte(0xFF07, 0x05); // TAC enabled, mode 01 (bit 3)
        timer.write_byte(0xFF06, 0x42); // TMA = 0x42
        timer.write_byte(0xFF05, 0xFF); // TIMA = 0xFF

        // Tick to cause TIMA overflow
        timer.update(16); // 16 ticks with mode 01 → 1 falling edge → TIMA 0xFF→overflow
        println!("After overflow: TIMA={:02X}, countdown={}", timer.read_byte(0xFF05), timer.overflow_countdown);
        // TIMA should be 0 (overflow just happened, countdown started)
        assert_eq!(timer.read_byte(0xFF05), 0);

        // Tick until reload fires (4 cycles)
        timer.update(3); // countdown 4→1
        println!("After 3 more ticks: TIMA={:02X}, reload_happened={}", timer.read_byte(0xFF05), timer.reload_happened);

        // The reload should fire on the 4th tick
        timer.update(1); // countdown 1→0 → reload fires
        println!("After reload: TIMA={:02X}, reload_happened={}", timer.read_byte(0xFF05), timer.reload_happened);
        assert_eq!(timer.read_byte(0xFF05), 0x42); // TMA loaded

        // Now simulate the cycle_write scenario: reload happened during tick_timer(3)
        // Reset to test the actual cycle_write flow
        let mut timer2 = Timer::new();
        timer2.write_byte(0xFF07, 0x05);
        timer2.write_byte(0xFF06, 0x42);
        timer2.write_byte(0xFF05, 0xFF);
        timer2.update(16); // overflow
        // Now simulate cycle_write(FF05, 0xAA): tick_timer(3) should trigger reload
        timer2.update(3); // 3 of the 4 countdown ticks
        println!("cycle_write sim - before final tick: countdown={}, reload_happened={}", timer2.overflow_countdown, timer2.reload_happened);
        timer2.update(1); // 4th tick → reload fires
        println!("cycle_write sim - after final tick: TIMA={:02X}, reload_happened={}", timer2.read_byte(0xFF05), timer2.reload_happened);
        // Now write to TIMA - should be IGNORED because reload just happened
        timer2.write_byte(0xFF05, 0xAA);
        println!("After write: TIMA={:02X}", timer2.read_byte(0xFF05));
        assert_eq!(timer2.read_byte(0xFF05), 0x42, "Write should be ignored when reload happened");
    }

    #[test]
    fn trace_ie_push() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/interrupts/ie_push.gb");
        if !std::path::Path::new(rom).exists() { return; }
        use crate::cpu::CPU; use crate::memory::MMU;
        let mut mmu = MMU::new(); mmu.load(rom).unwrap();
        let mut cpu = CPU::new_post_boot();
        mmu.write_byte(0xFF05, 0); mmu.write_byte(0xFF06, 0); mmu.write_byte(0xFF07, 0);
        mmu.write_byte(0xFF40, 0x91); mmu.write_byte(0xFF47, 0xFC); mmu.timer.set_div(0xABCC);
        let mut last_pcs = Vec::new();
        for i in 0..200000u32 {
            cpu.handle_interrupts(&mut mmu);
            cpu.step(&mut mmu);
            if i > 199900 { last_pcs.push(cpu.get_pc()); }
        }
        let unique: std::collections::HashSet<u16> = last_pcs.iter().copied().collect();
        println!("ie_push: {} unique PCs in last 100: {:04X?}", unique.len(), &last_pcs[..20.min(last_pcs.len())]);
        println!("Serial: {:?}", &mmu.serial_output);
    }

    #[test]
    fn trace_dmg_sound_01() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        use crate::cpu::CPU; use crate::memory::MMU;
        let mut mmu = MMU::new(); mmu.load(rom).unwrap();
        let mut cpu = CPU::new_post_boot();
        mmu.write_byte(0xFF40, 0x91); mmu.write_byte(0xFF47, 0xFC); mmu.timer.set_div(0xABCC);
        for _ in 0..20_000_000u32 {
            cpu.handle_interrupts(&mut mmu);
            cpu.step(&mut mmu);
        }
        let text: String = mmu.serial_output.iter().map(|&b| b as char).collect();
        println!("dmg_sound 01: '{}'", text.trim());
    }

    #[test]
    fn trace_dmg_sound_01_v2() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(5), |m, _| {
            let out = String::from_utf8_lossy(&m.serial_output);
            out.contains("Passed") || out.contains("Failed")
        });
        let out = String::from_utf8_lossy(&mmu.serial_output);
        println!("01-registers: '{}' ({} bytes)", &out[..out.len().min(200)], mmu.serial_output.len());
    }

    #[test]
    fn trace_dmg_sound_01_debug() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for batch in 0..10u32 {
            for _ in 0..500_000u32 {
                cpu.handle_interrupts(&mut mmu);
                cpu.step(&mut mmu);
            }
            let pc = cpu.get_pc();
            let ie = mmu.interrupts.ie;
            let if_ = mmu.interrupts.if_;
            let serial_len = mmu.serial_output.len();
            if batch == 0 || batch == 9 {
                println!("batch {}: PC={:04X} IE={:02X} IF={:02X} serial_len={}", batch, pc, ie, if_, serial_len);
            }
        }
    }

    #[test]
    fn trace_dmg_sound_01_early() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        // Trace first 1000 steps
        let mut halt_count = 0u32;
        for i in 0..100_000u32 {
            cpu.handle_interrupts(&mut mmu);
            cpu.step(&mut mmu);
            if cpu.is_halted() { halt_count += 1; }
            if i < 20 || (i % 10000 == 0) {
                let pc = cpu.get_pc();
                let ie = mmu.interrupts.ie;
                if i < 20 { println!("step {}: PC={:04X} IE={:02X}", i, pc, ie); }
            }
        }
        println!("halt_count={} IE={:02X} IF={:02X} serial_len={}", 
            halt_count, mmu.interrupts.ie, mmu.interrupts.if_, mmu.serial_output.len());
    }

    #[test]
    fn trace_dmg_sound_01_loop() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..2_000_000u32 {
            cpu.handle_interrupts(&mut mmu);
            cpu.step(&mut mmu);
        }
        println!("After 2M: PC={:04X} IE={:02X} IF={:02X} serial={}", 
            cpu.get_pc(), mmu.interrupts.ie, mmu.interrupts.if_, mmu.serial_output.len());
        for _ in 0..10_000_000u32 {
            cpu.handle_interrupts(&mut mmu);
            cpu.step(&mut mmu);
        }
        println!("After 12M: PC={:04X} IE={:02X} IF={:02X} serial={}", 
            cpu.get_pc(), mmu.interrupts.ie, mmu.interrupts.if_, mmu.serial_output.len());
    }

    #[test]
    fn trace_dmg_sound_combined() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/dmg_sound.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..5_000_000u32 {
            cpu.handle_interrupts(&mut mmu);
            cpu.step(&mut mmu);
        }
        let out = String::from_utf8_lossy(&mmu.serial_output);
        println!("combined serial ({} bytes): '{}'", mmu.serial_output.len(), &out[..out.len().min(100)]);
    }

    #[test]
    fn check_serial_mechanism() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/cpu_instrs/individual/01-special.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..5_000_000u32 {
            cpu.handle_interrupts(&mut mmu);
            cpu.step(&mut mmu);
        }
        let out = String::from_utf8_lossy(&mmu.serial_output);
        println!("cpu_instrs serial ({} bytes): '{}'", mmu.serial_output.len(), &out[..out.len().min(100)]);
    }

    #[test]
    fn trace_dmg_sound_exit() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        let mut last_pc = 0u16;
        for i in 0..2_000_000u32 {
            cpu.handle_interrupts(&mut mmu);
            cpu.step(&mut mmu);
            let pc = cpu.get_pc();
            if pc == 0xCAA2 && last_pc != 0xCAA2 {
                println!("Reached done loop at step {} from PC={:04X}", i, last_pc);
                // Check what's at wram for test result
                let result = mmu.read_byte(0xA000);
                let wram_c000 = mmu.read_byte(0xC000);
                println!("  0xA000={:02X} 0xC000={:02X}", result, wram_c000);
                break;
            }
            last_pc = pc;
        }
    }

    #[test]
    fn trace_dmg_sound_result() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        // Blargg tests store result text in WRAM starting around 0xD000 or similar
        // Check 0xA000 (external RAM if cart has it)  
        // Also check common result locations
        let a_val = cpu.get_a();
        println!("A={:02X} B={:02X} C={:02X}", a_val, cpu.get_b(), cpu.get_c());
        // Blargg test result is typically stored via serial or at memory
        // Let's scan WRAM for ASCII text
        let mut text = String::new();
        for addr in 0xC000..0xD000u16 {
            let b = mmu.read_byte(addr);
            if b >= 0x20 && b < 0x7F { text.push(b as char); }
            else if b == 0x0A { text.push('\n'); }
            else if !text.is_empty() && b == 0 { break; }
        }
        if !text.is_empty() { println!("WRAM text: '{}'", &text[..text.len().min(200)]); }
    }

    #[test]
    fn trace_dmg_sound_sram() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        // Blargg SRAM format: 0xA000-0xA003 = magic bytes, 0xA004 = result code, 0xA005+ = text
        let magic: Vec<u8> = (0xA000..0xA004).map(|a| mmu.read_byte(a)).collect();
        let result = mmu.read_byte(0xA004);
        let mut text = String::new();
        for addr in 0xA005..0xA100u16 {
            let b = mmu.read_byte(addr);
            if b == 0 { break; }
            text.push(b as char);
        }
        println!("SRAM magic={:02X?} result={:02X} text='{}'", magic, result, text);
    }

    #[test]
    fn trace_dmg_sound_full_text() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        let status = mmu.read_byte(0xA000);
        let mut text = String::new();
        for addr in 0xA004..0xA200u16 {
            let b = mmu.read_byte(addr);
            if b == 0 { break; }
            text.push(b as char);
        }
        println!("STATUS={} TEXT='{}'", status, text);
    }

    #[test]
    fn trace_dmg_sound_raw() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        let bytes: Vec<u8> = (0xA000..0xA040).map(|a| mmu.read_byte(a)).collect();
        println!("RAW: {:02X?}", &bytes);
        // Also show as ASCII where possible
        let s: String = bytes[4..].iter().map(|&b| if b >= 0x20 && b < 0x7F { b as char } else if b == 0x0A { '\n' } else { '.' }).collect();
        println!("TEXT: '{}'", s);
    }

    #[test]
    fn trace_dmg_sound_full_sram() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        let mut text = String::new();
        for addr in 0xA004..0xA300u16 {
            let b = mmu.read_byte(addr);
            if b == 0 { break; }
            if b >= 0x20 && b < 0x7F { text.push(b as char); }
            else if b == 0x0A { text.push('\n'); }
            else { text.push('.'); }
        }
        println!("FULL: '{}'", text);
    }

    #[test]
    fn check_apu_regs() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (_, mmu) = crate::init_dmg(rom);
        // Expected OR masks for reads (from Pan Docs)
        let expected_masks: [(u16, u8); 17] = [
            (0xFF10, 0x80), (0xFF11, 0x3F), (0xFF12, 0x00), (0xFF13, 0xFF), (0xFF14, 0xBF),
            (0xFF16, 0x3F), (0xFF17, 0x00), (0xFF18, 0xFF), (0xFF19, 0xBF),
            (0xFF1A, 0x7F), (0xFF1B, 0xFF), (0xFF1C, 0x9F), (0xFF1D, 0xFF), (0xFF1E, 0xBF),
            (0xFF20, 0xFF), (0xFF21, 0x00), (0xFF22, 0x00),
        ];
        for (addr, mask) in expected_masks {
            let val = mmu.read_byte(addr);
            if val & mask != mask {
                println!("FAIL {:04X}: read {:02X}, expected OR mask {:02X}", addr, val, mask);
            }
        }
        // Check NR52
        let nr52 = mmu.read_byte(0xFF26);
        println!("NR52={:02X} (expect F1 or 80+)", nr52);
        // Check unused regs return 0xFF  
        for addr in [0xFF15u16, 0xFF1F, 0xFF27, 0xFF28, 0xFF29] {
            let v = mmu.read_byte(addr);
            if v != 0xFF { println!("UNUSED {:04X}={:02X} (expect FF)", addr, v); }
        }
    }

    #[test]
    fn check_apu_write_read() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (_, mut mmu) = crate::init_dmg(rom);
        // Write 0x00 to all regs, read back, check OR masks
        let regs: [(u16, u8); 22] = [
            (0xFF10, 0x80), (0xFF11, 0x3F), (0xFF12, 0x00), (0xFF13, 0xFF), (0xFF14, 0xBF),
            (0xFF15, 0xFF), (0xFF16, 0x3F), (0xFF17, 0x00), (0xFF18, 0xFF), (0xFF19, 0xBF),
            (0xFF1A, 0x7F), (0xFF1B, 0xFF), (0xFF1C, 0x9F), (0xFF1D, 0xFF), (0xFF1E, 0xBF),
            (0xFF1F, 0xFF), (0xFF20, 0xFF), (0xFF21, 0x00), (0xFF22, 0x00), (0xFF23, 0xBF),
            (0xFF24, 0x00), (0xFF25, 0x00),
        ];
        for (addr, expected_or) in regs {
            mmu.write_byte(addr, 0x00);
            let readback = mmu.read_byte(addr);
            if readback != expected_or {
                println!("  {:04X}: wrote 0x00, read {:02X}, expected {:02X}", addr, readback, expected_or);
            }
        }
        // Write 0xFF, read back
        for (addr, expected_or) in regs {
            mmu.write_byte(addr, 0xFF);
            let readback = mmu.read_byte(addr);
            if readback != 0xFF {
                println!("  {:04X}: wrote 0xFF, read {:02X}, expected FF", addr, readback);
            }
        }
    }

    #[test]
    fn mooneye_acceptance_failures() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/");
        let path = std::path::Path::new(dir);
        if !path.exists() { return; }
        let mut passed = Vec::new();
        let mut failed = Vec::new();
        for entry in std::fs::read_dir(path).unwrap() {
            let p = entry.unwrap().path();
            if p.extension().map_or(true, |e| e != "gb") { continue; }
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if name.contains("-GS") || name.contains("-C.") || name.contains("-S.") { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(2), |m, steps| {
                let b = &m.serial_output;
                steps > 5_000_000 || b.windows(6).any(|w| w == [3,5,8,13,21,34] || w == [66,66,66,66,66,66])
            });
            let b = &mmu.serial_output;
            if b.windows(6).any(|w| w == [3,5,8,13,21,34]) { passed.push(name); }
            else { failed.push(name); }
        }
        passed.sort(); failed.sort();
        println!("PASS ({}/{}): {:?}", passed.len(), passed.len()+failed.len(), &passed);
        println!("FAIL: {:?}", &failed);
    }

    #[test]
    fn trace_if_ie() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/if_ie_registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(2), |m, _| {
            m.serial_output.len() >= 6
        });
        println!("if_ie: serial={:02X?}", &mmu.serial_output);
    }

    #[test]
    fn trace_rapid_di_ei() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/rapid_di_ei.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(2), |m, _| {
            m.serial_output.len() >= 6
        });
        let pass = mmu.serial_output.windows(6).any(|w| w == [3,5,8,13,21,34]);
        println!("rapid_di_ei: {} serial={:02X?}", if pass {"PASS"} else {"FAIL"}, &mmu.serial_output);
    }

    #[test]
    fn trace_ei_sequence() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/ei_sequence.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(2), |m, _| {
            m.serial_output.len() >= 6
        });
        let pass = mmu.serial_output.windows(6).any(|w| w == [3,5,8,13,21,34]);
        println!("ei_sequence: {} serial={:02X?}", if pass {"PASS"} else {"FAIL"}, &mmu.serial_output);
    }

    #[test]
    fn gbmicrotest_category_report() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        let path = std::path::Path::new(dir);
        if !path.exists() { return; }
        let mut categories: std::collections::HashMap<String, (u32, u32)> = std::collections::HashMap::new();
        for entry in std::fs::read_dir(path).unwrap() {
            let p = entry.unwrap().path();
            if p.extension().map_or(true, |e| e != "gb") { continue; }
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            // Extract category: everything before the last _X.gb or _XX.gb
            let cat = name.trim_end_matches(".gb")
                .trim_end_matches(|c: char| c.is_ascii_digit())
                .trim_end_matches('_')
                .to_string();
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let pass = mmu.read_byte(0xFF82) == 0x01;
            let e = categories.entry(cat).or_insert((0, 0));
            e.1 += 1;
            if pass { e.0 += 1; }
        }
        let mut cats: Vec<_> = categories.into_iter().collect();
        cats.sort_by(|a, b| (b.1.1 - b.1.0).cmp(&(a.1.1 - a.1.0))); // sort by failures desc
        for (cat, (pass, total)) in cats.iter().take(20) {
            if *pass < *total {
                println!("{:40} {}/{}", cat, pass, total);
            }
        }
    }

    #[test]
    fn measure_mode3_length() {
        let rom_path = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/hblank_int_scx0.gb");
        if !std::path::Path::new(rom_path).exists() { return; }
        // Measure mode 3 from actual emulation: run PPU from mode 2→3→0 and count
        let (mut cpu, mut mmu) = crate::init_dmg(rom_path);
        // Run until we're in mode 3
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        // Now measure: set up PPU at start of mode 2, count dots until mode 0
        mmu.ppu.scroll_x = 0;
        mmu.ppu.current_mode = 2;
        mmu.ppu.mode_clock = 0;
        let mut dots = 0u32;
        loop {
            mmu.ppu.update(1);
            dots += 1;
            if mmu.ppu.current_mode == 0 { break; }
            if dots > 600 { break; }
        }
        println!("Mode 2+3 with SCX=0: {} dots (expected ~252)", dots);

        mmu.ppu.scroll_x = 4;
        mmu.ppu.current_mode = 2;
        mmu.ppu.mode_clock = 0;
        dots = 0;
        loop {
            mmu.ppu.update(1);
            dots += 1;
            if mmu.ppu.current_mode == 0 { break; }
            if dots > 600 { break; }
        }
        println!("Mode 2+3 with SCX=4: {} dots (expected ~256)", dots);
    }

    #[test]
    fn hblank_scx_check() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for i in 0..8 {
            let path = format!("{}hblank_int_scx{}.gb", dir, i);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let pass = mmu.read_byte(0xFF82) == 0x01;
            let actual = mmu.read_byte(0xFF80);
            let expected = mmu.read_byte(0xFF81);
            println!("hblank_int_scx{}: {} (actual={:02X} expected={:02X})", i, if pass {"PASS"} else {"FAIL"}, actual, expected);
        }
    }

    #[test]
    fn test_jp_timing() {
        for name in ["jp_timing", "call_timing", "ret_timing", "reti_timing", "jp_cc_timing", "call_cc_timing", "ret_cc_timing"] {
            let rom = format!("{}/roms/test-suite/mooneye-test-suite/acceptance/{}.gb", env!("CARGO_MANIFEST_DIR"), name);
            let p = std::path::Path::new(&rom);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(3), |m, _| {
                m.serial_output.len() >= 6
            });
            let pass = mmu.serial_output.windows(6).any(|w| w == [3,5,8,13,21,34]);
            println!("{}: {}", name, if pass {"PASS"} else {"FAIL"});
        }
    }

    #[test]
    fn halt_tests() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for name in ["halt_bug.gb", "halt_op_dupe.gb", "halt_op_dupe_delay.gb"] {
            let path = format!("{}{}", dir, name);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let pass = mmu.read_byte(0xFF82) == 0x01;
            let actual = mmu.read_byte(0xFF80);
            let expected = mmu.read_byte(0xFF81);
            println!("{}: {} (actual={:02X} expected={:02X})", name, if pass {"PASS"} else {"FAIL"}, actual, expected);
        }
    }

    #[test]
    fn halt_bug_detail() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/halt_bug.gb");
        if !std::path::Path::new(path).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(path);
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        println!("FF80={:02X} FF81={:02X} FF82={:02X}", mmu.read_byte(0xFF80), mmu.read_byte(0xFF81), mmu.read_byte(0xFF82));
    }

    #[test]
    fn trace_boot_hwio() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/boot_hwio-dmgABCmgb.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(3), |m, _| m.serial_output.len() >= 6);
        let pass = mmu.serial_output.windows(6).any(|w| w == [3,5,8,13,21,34]);
        println!("boot_hwio: {}", if pass {"PASS"} else {"FAIL"});
        if !pass && mmu.serial_output.len() > 6 {
            // Output might contain register values
            println!("  serial: {:02X?}", &mmu.serial_output[..mmu.serial_output.len().min(20)]);
        }
    }

    #[test]
    fn check_boot_init_regs() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/boot_hwio-dmgABCmgb.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (_, mmu) = crate::init_dmg(rom);
        // Expected values from Pan Docs for DMG
        let checks: &[(u16, u8, &str)] = &[
            (0xFF00, 0xCF, "P1"), (0xFF01, 0x00, "SB"), (0xFF02, 0x7E, "SC"),
            (0xFF05, 0x00, "TIMA"), (0xFF06, 0x00, "TMA"), (0xFF07, 0xF8, "TAC"),
            (0xFF0F, 0xE1, "IF"), // Should be 0xE1 after boot!
            (0xFF10, 0x80, "NR10"), (0xFF11, 0xBF, "NR11"), (0xFF12, 0xF3, "NR12"),
            (0xFF14, 0xBF, "NR14"), (0xFF16, 0x3F, "NR21"),
            (0xFF24, 0x77, "NR50"), (0xFF25, 0xF3, "NR51"), (0xFF26, 0xF1, "NR52"),
            (0xFF40, 0x91, "LCDC"), (0xFF42, 0x00, "SCY"), (0xFF43, 0x00, "SCX"),
            (0xFF45, 0x00, "LYC"), (0xFF47, 0xFC, "BGP"),
            (0xFF4A, 0x00, "WY"), (0xFF4B, 0x00, "WX"),
        ];
        for (addr, expected, name) in checks {
            let actual = mmu.read_byte(*addr);
            if actual != *expected {
                println!("MISMATCH {}: {:04X} = {:02X} (expected {:02X})", name, addr, actual, expected);
            }
        }
    }

    #[test]
    fn check_more_init_regs() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/boot_hwio-dmgABCmgb.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (_, mmu) = crate::init_dmg(rom);
        // Check registers that Pan Docs specifies for DMG
        let more: &[(u16, u8, &str)] = &[
            (0xFF04, 0xAB, "DIV"), // top byte of internal counter
            (0xFF41, 0x85, "STAT"), // mode 1 + LYC=LY flag
            (0xFF44, 0x00, "LY"), 
            (0xFF46, 0xFF, "DMA"),
            (0xFF48, 0xFF, "OBP0"), // uninitialized
            (0xFF49, 0xFF, "OBP1"), // uninitialized
            (0xFFFF, 0x00, "IE"),
        ];
        for (addr, expected, name) in more {
            let actual = mmu.read_byte(*addr);
            if actual != *expected {
                println!("  {}: {:04X} = {:02X} (expected {:02X})", name, addr, actual, expected);
            }
        }
    }

    #[test]
    fn dma_tests() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for name in ["dma_basic.gb", "dma_0x1000.gb", "dma_0x9000.gb", "dma_0xA000.gb", "dma_0xC000.gb", "dma_0xE000.gb", "dma_timing_a.gb"] {
            let path = format!("{}{}", dir, name);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let pass = mmu.read_byte(0xFF82) == 0x01;
            let actual = mmu.read_byte(0xFF80);
            let expected = mmu.read_byte(0xFF81);
            println!("{}: {} (a={:02X} e={:02X})", name, if pass {"PASS"} else {"FAIL"}, actual, expected);
        }
    }

    #[test]
    fn int_hblank_detailed() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for pattern in ["int_hblank_halt_scx", "int_hblank_nops_scx", "int_hblank_incs_scx"] {
            for i in 0..8 {
                let path = format!("{}{}{}.gb", dir, pattern, i);
                let p = std::path::Path::new(&path);
                if !p.exists() { continue; }
                let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
                for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
                let pass = mmu.read_byte(0xFF82) == 0x01;
                if !pass {
                    let a = mmu.read_byte(0xFF80);
                    let e = mmu.read_byte(0xFF81);
                    println!("FAIL {}{}: a={:02X} e={:02X} diff={}", pattern, i, a, e, (e as i16 - a as i16));
                }
            }
        }
    }

    #[test]
    fn poweron_tests() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for name in ["poweron_ly_000.gb", "poweron_ly_001.gb", "poweron_ly_002.gb", "poweron_ly_003.gb", "poweron_ly_004.gb"] {
            let path = format!("{}{}", dir, name);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let pass = mmu.read_byte(0xFF82) == 0x01;
            let a = mmu.read_byte(0xFF80);
            let e = mmu.read_byte(0xFF81);
            if !pass { println!("{}: a={:02X} e={:02X}", name, a, e); }
        }
    }

    #[test]
    fn test_dmg_sound_02() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/02-len ctr.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..1_000_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        let status = mmu.read_byte(0xA000);
        let sig = [mmu.read_byte(0xA001), mmu.read_byte(0xA002), mmu.read_byte(0xA003)];
        let mut text = String::new();
        for addr in 0xA004..0xA100u16 {
            let b = mmu.read_byte(addr);
            if b == 0 { break; }
            if b >= 0x20 && b < 0x7F { text.push(b as char); }
            else if b == 0x0A { text.push('\n'); }
        }
        println!("02-len: status={} sig={:02X?} text='{}'", status, sig, text);
    }

    #[test]
    fn test_dmg_sound_02_long() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/02-len ctr.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..10_000_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        let status = mmu.read_byte(0xA000);
        let mut text = String::new();
        for addr in 0xA004..0xA200u16 {
            let b = mmu.read_byte(addr);
            if b == 0 { break; }
            if b >= 0x20 && b < 0x7F { text.push(b as char); }
            else if b == 0x0A { text.push('\n'); }
        }
        println!("02-len: status={} text='{}'", status, text);
    }

    #[test]
    fn check_02_sram_sig() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/02-len ctr.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(5), |m, steps| {
            let sig = [m.read_byte(0xA001), m.read_byte(0xA002), m.read_byte(0xA003)];
            (sig == [0xDE, 0xB0, 0x61] && m.read_byte(0xA000) != 0x80) || steps >= 10_000_000
        });
        let sig = [mmu.read_byte(0xA001), mmu.read_byte(0xA002), mmu.read_byte(0xA003)];
        let status = mmu.read_byte(0xA000);
        println!("02 SRAM: status={:02X} sig={:02X?} pass={}", status, sig, status == 0 && sig == [0xDE, 0xB0, 0x61]);
    }

    #[test]
    fn test_dmg_sound_03() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/03-trigger.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(10), |m, steps| {
            let sig = [m.read_byte(0xA001), m.read_byte(0xA002), m.read_byte(0xA003)];
            (sig == [0xDE, 0xB0, 0x61] && m.read_byte(0xA000) != 0x80) || steps >= 30_000_000
        });
        let status = mmu.read_byte(0xA000);
        let mut text = String::new();
        for addr in 0xA004..0xA200u16 {
            let b = mmu.read_byte(addr);
            if b == 0 { break; }
            if b >= 0x20 && b < 0x7F { text.push(b as char); }
            else if b == 0x0A { text.push('\n'); }
        }
        println!("03-trigger: status={} text='{}'", status, text);
    }

    #[test]
    fn test_dmg_sound_05() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/05-sweep details.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(10), |m, steps| {
            let sig = [m.read_byte(0xA001), m.read_byte(0xA002), m.read_byte(0xA003)];
            (sig == [0xDE, 0xB0, 0x61] && m.read_byte(0xA000) != 0x80) || steps >= 30_000_000
        });
        let status = mmu.read_byte(0xA000);
        let mut text = String::new();
        for addr in 0xA004..0xA200u16 {
            let b = mmu.read_byte(addr);
            if b == 0 { break; }
            if b >= 0x20 && b < 0x7F { text.push(b as char); }
            else if b == 0x0A { text.push('\n'); }
        }
        println!("05-sweep: status={} text='{}'", status, text);
    }

    #[test]
    fn test_dmg_sound_11() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/11-regs after power.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(10), |m, steps| {
            let sig = [m.read_byte(0xA001), m.read_byte(0xA002), m.read_byte(0xA003)];
            (sig == [0xDE, 0xB0, 0x61] && m.read_byte(0xA000) != 0x80) || steps >= 30_000_000
        });
        let status = mmu.read_byte(0xA000);
        println!("11-regs: status={}", status);
    }

    #[test]
    fn test_all_dmg_sound() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/");
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.extension().map_or(true, |e| e != "gb") { continue; }
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(10), |m, steps| {
                let sig = [m.read_byte(0xA001), m.read_byte(0xA002), m.read_byte(0xA003)];
                (sig == [0xDE, 0xB0, 0x61] && m.read_byte(0xA000) != 0x80) || steps >= 30_000_000
            });
            let status = mmu.read_byte(0xA000);
            if status == 0 { println!("{}: PASS", name); }
            else { println!("{}: FAIL #{}", name, status); }
        }
    }

    #[test]
    fn poweron_oam_detail() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for i in 0..14 {
            let name = format!("poweron_oam_{:03}.gb", i);
            let path = format!("{}{}", dir, name);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let pass = mmu.read_byte(0xFF82) == 0x01;
            if !pass {
                let a = mmu.read_byte(0xFF80);
                let e = mmu.read_byte(0xFF81);
                println!("{}: a={:02X} e={:02X}", name, a, e);
            }
        }
    }

    #[test]
    fn test_halt_ime0() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/halt_ime0_nointr_timing.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(3), |m, _| m.serial_output.len() >= 6);
        let pass = mmu.serial_output.windows(6).any(|w| w == [3,5,8,13,21,34]);
        println!("halt_ime0_nointr: {}", if pass {"PASS"} else {"FAIL"});
    }

    #[test]
    fn test_oam_dma_start() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/oam_dma_start.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(3), |m, _| m.serial_output.len() >= 6);
        let pass = mmu.serial_output.windows(6).any(|w| w == [3,5,8,13,21,34]);
        println!("oam_dma_start: {}", if pass {"PASS"} else {"FAIL"});
    }

    #[test]
    fn test_age_dmg() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/age-test-roms/");
        for f in ["oam/oam-write-dmgC.gb", "stat-mode/stat-mode-dmgC-cgbBC.gb", "ly/ly-dmgC-cgbBC.gb", "halt/halt-prefetch-dmgC-cgbBCE.gb"] {
            let path = format!("{}{}", dir, f);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(3), |m, steps| {
                m.serial_output.len() >= 6 || steps >= 5_000_000
            });
            let pass = mmu.serial_output.windows(6).any(|w| w == [3,5,8,13,21,34]);
            println!("{}: {}", f.split('/').last().unwrap(), if pass {"PASS"} else {"FAIL"});
        }
    }

    #[test]
    fn lcdon_hblank_tests() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for name in ["hblank_int_l0.gb", "hblank_int_l1.gb", "hblank_int_l2.gb",
                     "lcdon_to_oam_int_l0.gb", "lcdon_to_oam_int_l1.gb", "lcdon_to_oam_int_l2.gb"] {
            let path = format!("{}{}", dir, name);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let pass = mmu.read_byte(0xFF82) == 0x01;
            let a = mmu.read_byte(0xFF80);
            let e = mmu.read_byte(0xFF81);
            if !pass { println!("{}: a={:02X} e={:02X}", name, a, e); }
            else { println!("{}: PASS", name); }
        }
    }

    #[test]
    fn gbmicrotest_near_misses() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        let path = std::path::Path::new(dir);
        if !path.exists() { return; }
        let mut near = Vec::new();
        for entry in std::fs::read_dir(path).unwrap() {
            let p = entry.unwrap().path();
            if p.extension().map_or(true, |e| e != "gb") { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            if mmu.read_byte(0xFF82) != 0x01 {
                let a = mmu.read_byte(0xFF80) as i16;
                let e = mmu.read_byte(0xFF81) as i16;
                let diff = (e - a).abs();
                if diff > 0 && diff <= 2 {
                    near.push((p.file_name().unwrap().to_string_lossy().to_string(), a as u8, e as u8));
                }
            }
        }
        near.sort();
        println!("Tests off by 1-2 ({}):", near.len());
        for (name, a, e) in &near { println!("  {}: a={:02X} e={:02X}", name, a, e); }
    }

    #[test]
    fn tima_boot_phase() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/004-tima_boot_phase.gb");
        if !std::path::Path::new(dir).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(dir);
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        let a = mmu.read_byte(0xFF80);
        let e = mmu.read_byte(0xFF81);
        let pass = mmu.read_byte(0xFF82) == 0x01;
        println!("tima_boot_phase: {} a={:02X} e={:02X}", if pass {"PASS"} else {"FAIL"}, a, e);
    }

    #[test]
    fn test_same_suite_dmg() {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/same-suite/");
        let tests = ["apu/div_write_trigger.gb", "apu/div_write_trigger_10.gb", "interrupt/ei_delay_halt.gb"];
        for t in tests {
            let path = format!("{}{}", base, t);
            let p = std::path::Path::new(&path);
            if !p.exists() { println!("{}: NOT FOUND", t); continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(5), |m, steps| {
                m.serial_output.len() >= 6 || steps >= 5_000_000
            });
            let pass = mmu.serial_output.windows(6).any(|w| w == [3,5,8,13,21,34]);
            println!("{}: {}", t, if pass {"PASS"} else {"FAIL"});
        }
    }

    #[test]
    fn test_mooneye_ppu() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/ppu/");
        for name in ["stat_irq_blocking.gb", "stat_lyc_onoff.gb", "intr_2_0_timing.gb", "intr_2_mode0_timing.gb", "intr_2_mode3_timing.gb", "intr_2_oam_ok_timing.gb", "intr_2_mode0_timing_sprites.gb"] {
            let path = format!("{}{}", dir, name);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(3), |m, _| m.serial_output.len() >= 6);
            let pass = mmu.serial_output.windows(6).any(|w| w == [3,5,8,13,21,34]);
            println!("{}: {}", name, if pass {"PASS"} else {"FAIL"});
        }
    }

    #[test]
    fn trace_stat_lyc_onoff() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/mooneye-test-suite/acceptance/ppu/stat_lyc_onoff.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        crate::run_until(&mut cpu, &mut mmu, std::time::Duration::from_secs(3), |m, _| m.serial_output.len() >= 6);
        println!("stat_lyc_onoff serial: {:02X?}", &mmu.serial_output);
    }

    #[test]
    fn gbmicrotest_logic_bugs() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        let path = std::path::Path::new(dir);
        if !path.exists() { return; }
        let mut bugs = Vec::new();
        for entry in std::fs::read_dir(path).unwrap() {
            let p = entry.unwrap().path();
            if p.extension().map_or(true, |e| e != "gb") { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            if mmu.read_byte(0xFF82) != 0x01 {
                let a = mmu.read_byte(0xFF80);
                let e = mmu.read_byte(0xFF81);
                let diff = (e as i16 - a as i16).unsigned_abs();
                if diff > 2 && diff < 200 && a != 0xFF && e != 0xFF && a != 0 && e != 0 {
                    bugs.push((p.file_name().unwrap().to_string_lossy().to_string(), a, e, diff));
                }
            }
        }
        bugs.sort_by_key(|x| x.3);
        println!("Logic bugs (diff > 2, < 200): {}", bugs.len());
        for (name, a, e, d) in &bugs { println!("  {}: a={:02X} e={:02X} diff={}", name, a, e, d); }
    }

    #[test]
    fn vram_write_tests() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for name in ["vram_write_l0_a.gb","vram_write_l0_b.gb","vram_write_l0_c.gb","vram_write_l0_d.gb",
                     "vram_write_l1_a.gb","vram_write_l1_b.gb","vram_write_l1_c.gb","vram_write_l1_d.gb"] {
            let path = format!("{}{}", dir, name);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let pass = mmu.read_byte(0xFF82) == 0x01;
            if !pass { let a = mmu.read_byte(0xFF80); let e = mmu.read_byte(0xFF81); println!("{}: FAIL a={:02X} e={:02X}", name, a, e); }
            else { println!("{}: PASS", name); }
        }
    }

    #[test]
    fn vblank_int_test() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for name in ["int_vblank1_nops.gb", "int_vblank1_halt.gb", "int_vblank2_nops.gb", "vblank_int_inc_sled.gb"] {
            let path = format!("{}{}", dir, name);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let a = mmu.read_byte(0xFF80); let e = mmu.read_byte(0xFF81);
            let pass = mmu.read_byte(0xFF82) == 0x01;
            println!("{}: {} a={:02X} e={:02X} diff={}", name, if pass {"PASS"} else {"FAIL"}, a, e, (e as i16 - a as i16));
        }
    }

    #[test]
    fn int_hblank_nops_detail() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        let path = format!("{}int_hblank_nops_scx0.gb", dir);
        let p = std::path::Path::new(&path);
        if !p.exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        let a = mmu.read_byte(0xFF80);
        let e = mmu.read_byte(0xFF81);
        let pass = mmu.read_byte(0xFF82) == 0x01;
        println!("int_hblank_nops_scx0: {} a={:02X} e={:02X} diff={}", if pass {"PASS"} else {"FAIL"}, a, e, e as i16 - a as i16);
    }

    #[test]
    fn scanline_length_check() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/int_hblank_nops_scx0.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        // Run until PPU is in a known state
        for _ in 0..100_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        // Record when LY changes
        let start_ly = mmu.ppu.ly;
        let start_clock = mmu.ppu.mode_clock;
        let start_mode = mmu.ppu.current_mode;
        println!("At check: LY={} mode={} mode_clock={}", start_ly, start_mode, start_clock);
    }

    #[test]
    fn mode3_end_timing() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/int_hblank_nops_scx0.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        mmu.ppu.scroll_x = 0;
        // Run until we're at the start of a line in mode 2
        for _ in 0..50_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        // Now find a mode 2 start (bounded so a phase mismatch can't spin forever)
        let mut guard = 0u32;
        while mmu.ppu.current_mode != 2 || mmu.ppu.mode_clock > 4 {
            cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu);
            guard += 1;
            if guard > 100_000 { return; }
        }
        let line = mmu.ppu.ly;
        // Run dot-by-dot through the PPU to find exact mode 3 end
        let mut mode3_end = 0u32;
        for dot in 0..460u32 {
            mmu.ppu.update(1);
            if mmu.ppu.current_mode == 0 && mode3_end == 0 {
                mode3_end = dot + mmu.ppu.mode_clock - 1;
                println!("LY={}: mode 3 ended at dot {} (mode_clock={})", line, dot, mmu.ppu.mode_clock);
                break;
            }
        }
    }

    #[test]
    fn check_sprite_count_hblank_test() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/int_hblank_nops_scx0.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        for _ in 0..50_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        // Check OAM for any sprites
        let mut sprite_count = 0;
        for i in 0..40 {
            let y = mmu.ppu.oam[i * 4];
            let x = mmu.ppu.oam[i * 4 + 1];
            if y > 0 && y < 160 && x > 0 && x < 168 { sprite_count += 1; }
        }
        println!("Sprites in OAM: {}", sprite_count);
        println!("LCDC: {:02X} (sprites enabled: {})", mmu.ppu.lcd_control, mmu.ppu.lcd_control & 0x02 != 0);
    }

    #[test]
    fn test_01_registers_detail() {
        let rom = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/blargg/dmg_sound/rom_singles/01-registers.gb");
        if !std::path::Path::new(rom).exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(rom);
        // Run until test completes
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        // Check NR52 status
        let nr52 = mmu.read_byte(0xFF26);
        println!("NR52={:02X}", nr52);
        // Read all APU registers to see if any are wrong
        let expected_on: &[(u16, u8)] = &[
            (0xFF10, 0x80), (0xFF11, 0x3F), (0xFF12, 0x00), (0xFF13, 0xFF), (0xFF14, 0xBF),
            (0xFF16, 0x3F), (0xFF17, 0x00), (0xFF18, 0xFF), (0xFF19, 0xBF),
            (0xFF1A, 0x7F), (0xFF1B, 0xFF), (0xFF1C, 0x9F), (0xFF1D, 0xFF), (0xFF1E, 0xBF),
            (0xFF20, 0xFF), (0xFF21, 0x00), (0xFF22, 0x00), (0xFF23, 0xBF),
        ];
        // When APU is on but channels reset, read should give OR masks
        // Turn APU off then on to get clean state
        mmu.write_byte(0xFF26, 0x00); // APU off
        mmu.write_byte(0xFF26, 0x80); // APU on
        for (addr, expected) in expected_on {
            let val = mmu.read_byte(*addr);
            if val != *expected {
                println!("  {:04X}: got {:02X} expected {:02X}", addr, val, expected);
            }
        }
    }

    #[test]
    fn timer_gbmicrotest() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        let mut pass = 0; let mut fail = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if !name.starts_with("timer_") || p.extension().map_or(true, |e| e != "gb") { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            if mmu.read_byte(0xFF82) == 0x01 { pass += 1; }
            else {
                fail += 1;
                let a = mmu.read_byte(0xFF80); let e = mmu.read_byte(0xFF81);
                if fail <= 5 { println!("FAIL {}: a={:02X} e={:02X}", name, a, e); }
            }
        }
        println!("Timer gbmicrotest: {}/{}", pass, pass + fail);
    }

    #[test]
    fn div_gbmicrotest() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        for name in ["div_inc_timing_a.gb", "div_inc_timing_b.gb"] {
            let path = format!("{}{}", dir, name);
            let p = std::path::Path::new(&path);
            if !p.exists() { continue; }
            let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
            for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
            let pass = mmu.read_byte(0xFF82) == 0x01;
            println!("{}: {}", name, if pass {"PASS"} else {"FAIL"});
        }
    }

    #[test]
    fn sprite4_check() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/roms/test-suite/gbmicrotest/");
        let path = format!("{}sprite4_0_a.gb", dir);
        let p = std::path::Path::new(&path);
        if !p.exists() { return; }
        let (mut cpu, mut mmu) = crate::init_dmg(p.to_str().unwrap());
        for _ in 0..500_000u32 { cpu.handle_interrupts(&mut mmu); cpu.step(&mut mmu); }
        // Check sprites
        let mut count = 0;
        for i in 0..40 {
            let y = mmu.ppu.oam[i * 4];
            let x = mmu.ppu.oam[i * 4 + 1];
            if y > 0 && y < 160 { count += 1; if count <= 4 { println!("  sprite {}: y={} x={}", i, y, x); } }
        }
        println!("LCDC={:02X} sprites_enabled={}", mmu.ppu.lcd_control, mmu.ppu.lcd_control & 0x02 != 0);
        println!("Total sprites in OAM: {}", count);
        let a = mmu.read_byte(0xFF80); let e = mmu.read_byte(0xFF81);
        println!("Result: a={:02X} e={:02X}", a, e);
    }
