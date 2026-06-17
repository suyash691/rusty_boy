use crate::memory::MMU;
use super::{CPU, CARRY_FLAG, HALF_CARRY_FLAG, SUBTRACT_FLAG, ZERO_FLAG};

impl CPU {
    pub(super) fn execute(&mut self, opcode: u8, memory: &mut MMU) {
        match opcode {
            0x00 => {} // NOP (fetch already ticked 4)
            0x01 => { let v = self.fetch_word(memory); self.set_bc(v); }
            0x02 => { let addr = self.get_bc(); self.write_byte(memory, addr, self.a); }
            0x03 => { let v = self.get_bc().wrapping_add(1); self.set_bc(v); self.internal_cycle(memory); }
            0x04 => { self.b = self.inc(self.b); }
            0x05 => { self.b = self.dec(self.b); }
            0x06 => { self.b = self.fetch(memory); }
            0x07 => { // RLCA
                let a = self.a;
                self.a = (a << 1) | (a >> 7);
                self.f = 0;
                if a & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
            }
            0x08 => { // LD (nn),SP
                let addr = self.fetch_word(memory);
                self.write_byte(memory, addr, (self.sp & 0xFF) as u8);
                self.write_byte(memory, addr.wrapping_add(1), (self.sp >> 8) as u8);
            }
            0x09 => { let v = self.get_bc(); self.add_hl(v); self.internal_cycle(memory); }
            0x0A => { let addr = self.get_bc(); self.a = self.read_byte(memory, addr); }
            0x0B => { let v = self.get_bc().wrapping_sub(1); self.set_bc(v); self.internal_cycle(memory); }
            0x0C => { self.c = self.inc(self.c); }
            0x0D => { self.c = self.dec(self.c); }
            0x0E => { self.c = self.fetch(memory); }
            0x0F => { // RRCA
                let a = self.a;
                self.a = (a >> 1) | (a << 7);
                self.f = 0;
                if a & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
            }

            0x10 => { memory.do_speed_switch(); self.stop = !memory.cgb_mode; }
            0x11 => { let v = self.fetch_word(memory); self.set_de(v); }
            0x12 => { let addr = self.get_de(); self.write_byte(memory, addr, self.a); }
            0x13 => { let v = self.get_de().wrapping_add(1); self.set_de(v); self.internal_cycle(memory); }
            0x14 => { self.d = self.inc(self.d); }
            0x15 => { self.d = self.dec(self.d); }
            0x16 => { self.d = self.fetch(memory); }
            0x17 => { // RLA
                let a = self.a;
                let c = self.is_flag_set(CARRY_FLAG) as u8;
                self.a = (a << 1) | c;
                self.f = 0;
                if a & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
            }
            0x18 => { let n = self.fetch(memory) as i8; self.pc = self.pc.wrapping_add(n as u16); self.internal_cycle(memory); }
            0x19 => { let v = self.get_de(); self.add_hl(v); self.internal_cycle(memory); }
            0x1A => { let addr = self.get_de(); self.a = self.read_byte(memory, addr); }
            0x1B => { let v = self.get_de().wrapping_sub(1); self.set_de(v); self.internal_cycle(memory); }
            0x1C => { self.e = self.inc(self.e); }
            0x1D => { self.e = self.dec(self.e); }
            0x1E => { self.e = self.fetch(memory); }
            0x1F => { // RRA
                let a = self.a;
                let c = (self.is_flag_set(CARRY_FLAG) as u8) << 7;
                self.a = (a >> 1) | c;
                self.f = 0;
                if a & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
            }

            0x20 => { let n = self.fetch(memory) as i8; if !self.is_flag_set(ZERO_FLAG) { self.pc = self.pc.wrapping_add(n as u16); self.internal_cycle(memory); } }
            0x21 => { let v = self.fetch_word(memory); self.set_hl(v); }
            0x22 => { let hl = self.get_hl(); self.write_byte(memory, hl, self.a); self.set_hl(hl.wrapping_add(1)); }
            0x23 => { let v = self.get_hl().wrapping_add(1); self.set_hl(v); self.internal_cycle(memory); }
            0x24 => { self.h = self.inc(self.h); }
            0x25 => { self.h = self.dec(self.h); }
            0x26 => { self.h = self.fetch(memory); }
            0x27 => { self.daa(); }
            0x28 => { let n = self.fetch(memory) as i8; if self.is_flag_set(ZERO_FLAG) { self.pc = self.pc.wrapping_add(n as u16); self.internal_cycle(memory); } }
            0x29 => { let v = self.get_hl(); self.add_hl(v); self.internal_cycle(memory); }
            0x2A => { let hl = self.get_hl(); self.a = self.read_byte(memory, hl); self.set_hl(hl.wrapping_add(1)); }
            0x2B => { let v = self.get_hl().wrapping_sub(1); self.set_hl(v); self.internal_cycle(memory); }
            0x2C => { self.l = self.inc(self.l); }
            0x2D => { self.l = self.dec(self.l); }
            0x2E => { self.l = self.fetch(memory); }
            0x2F => { self.a = !self.a; self.set_flag(SUBTRACT_FLAG); self.set_flag(HALF_CARRY_FLAG); }

            0x30 => { let n = self.fetch(memory) as i8; if !self.is_flag_set(CARRY_FLAG) { self.pc = self.pc.wrapping_add(n as u16); self.internal_cycle(memory); } }
            0x31 => { self.sp = self.fetch_word(memory); }
            0x32 => { let hl = self.get_hl(); self.write_byte(memory, hl, self.a); self.set_hl(hl.wrapping_sub(1)); }
            0x33 => { self.sp = self.sp.wrapping_add(1); self.internal_cycle(memory); }
            0x34 => { let addr = self.get_hl(); let v = self.read_byte(memory, addr); let r = self.inc(v); self.write_byte(memory, addr, r); }
            0x35 => { let addr = self.get_hl(); let v = self.read_byte(memory, addr); let r = self.dec(v); self.write_byte(memory, addr, r); }
            0x36 => { let v = self.fetch(memory); let addr = self.get_hl(); self.write_byte(memory, addr, v); }
            0x37 => { self.clear_flag(SUBTRACT_FLAG); self.clear_flag(HALF_CARRY_FLAG); self.set_flag(CARRY_FLAG); }
            0x38 => { let n = self.fetch(memory) as i8; if self.is_flag_set(CARRY_FLAG) { self.pc = self.pc.wrapping_add(n as u16); self.internal_cycle(memory); } }
            0x39 => { let v = self.sp; self.add_hl(v); self.internal_cycle(memory); }
            0x3A => { let hl = self.get_hl(); self.a = self.read_byte(memory, hl); self.set_hl(hl.wrapping_sub(1)); }
            0x3B => { self.sp = self.sp.wrapping_sub(1); self.internal_cycle(memory); }
            0x3C => { self.a = self.inc(self.a); }
            0x3D => { self.a = self.dec(self.a); }
            0x3E => { self.a = self.fetch(memory); }
            0x3F => { self.clear_flag(SUBTRACT_FLAG); self.clear_flag(HALF_CARRY_FLAG); if self.is_flag_set(CARRY_FLAG) { self.clear_flag(CARRY_FLAG); } else { self.set_flag(CARRY_FLAG); } }

            // LD r,r'  (0x40 LD B,B is a NOP; the debugger handles it as a breakpoint opcode)
            0x40 => {} 0x41 => { self.b = self.c; } 0x42 => { self.b = self.d; } 0x43 => { self.b = self.e; }
            0x44 => { self.b = self.h; } 0x45 => { self.b = self.l; }
            0x46 => { let addr = self.get_hl(); self.b = self.read_byte(memory, addr); } 0x47 => { self.b = self.a; }
            0x48 => { self.c = self.b; } 0x49 => {} 0x4A => { self.c = self.d; } 0x4B => { self.c = self.e; }
            0x4C => { self.c = self.h; } 0x4D => { self.c = self.l; }
            0x4E => { let addr = self.get_hl(); self.c = self.read_byte(memory, addr); } 0x4F => { self.c = self.a; }
            0x50 => { self.d = self.b; } 0x51 => { self.d = self.c; } 0x52 => {} 0x53 => { self.d = self.e; }
            0x54 => { self.d = self.h; } 0x55 => { self.d = self.l; }
            0x56 => { let addr = self.get_hl(); self.d = self.read_byte(memory, addr); } 0x57 => { self.d = self.a; }
            0x58 => { self.e = self.b; } 0x59 => { self.e = self.c; } 0x5A => { self.e = self.d; } 0x5B => {}
            0x5C => { self.e = self.h; } 0x5D => { self.e = self.l; }
            0x5E => { let addr = self.get_hl(); self.e = self.read_byte(memory, addr); } 0x5F => { self.e = self.a; }
            0x60 => { self.h = self.b; } 0x61 => { self.h = self.c; } 0x62 => { self.h = self.d; } 0x63 => { self.h = self.e; }
            0x64 => {} 0x65 => { self.h = self.l; }
            0x66 => { let addr = self.get_hl(); self.h = self.read_byte(memory, addr); } 0x67 => { self.h = self.a; }
            0x68 => { self.l = self.b; } 0x69 => { self.l = self.c; } 0x6A => { self.l = self.d; } 0x6B => { self.l = self.e; }
            0x6C => { self.l = self.h; } 0x6D => {}
            0x6E => { let addr = self.get_hl(); self.l = self.read_byte(memory, addr); } 0x6F => { self.l = self.a; }
            0x70 => { let addr = self.get_hl(); self.write_byte(memory, addr, self.b); }
            0x71 => { let addr = self.get_hl(); self.write_byte(memory, addr, self.c); }
            0x72 => { let addr = self.get_hl(); self.write_byte(memory, addr, self.d); }
            0x73 => { let addr = self.get_hl(); self.write_byte(memory, addr, self.e); }
            0x74 => { let addr = self.get_hl(); self.write_byte(memory, addr, self.h); }
            0x75 => { let addr = self.get_hl(); self.write_byte(memory, addr, self.l); }
            0x76 => { // HALT
                if self.ime {
                    self.halt = true;
                } else if memory.interrupts.pending() != 0 {
                    // HALT bug: next fetch won't increment PC
                    self.halt_bug = true;
                } else {
                    self.halt = true;
                }
            }
            0x77 => { let addr = self.get_hl(); self.write_byte(memory, addr, self.a); }
            0x78 => { self.a = self.b; } 0x79 => { self.a = self.c; }
            0x7A => { self.a = self.d; } 0x7B => { self.a = self.e; }
            0x7C => { self.a = self.h; } 0x7D => { self.a = self.l; }
            0x7E => { let addr = self.get_hl(); self.a = self.read_byte(memory, addr); }
            0x7F => {}

            // ALU r
            0x80 => { self.add_a(self.b); } 0x81 => { self.add_a(self.c); }
            0x82 => { self.add_a(self.d); } 0x83 => { self.add_a(self.e); }
            0x84 => { self.add_a(self.h); } 0x85 => { self.add_a(self.l); }
            0x86 => { let v = self.read_byte(memory, self.get_hl()); self.add_a(v); }
            0x87 => { self.add_a(self.a); }
            0x88 => { self.adc_a(self.b); } 0x89 => { self.adc_a(self.c); }
            0x8A => { self.adc_a(self.d); } 0x8B => { self.adc_a(self.e); }
            0x8C => { self.adc_a(self.h); } 0x8D => { self.adc_a(self.l); }
            0x8E => { let v = self.read_byte(memory, self.get_hl()); self.adc_a(v); }
            0x8F => { self.adc_a(self.a); }
            0x90 => { self.sub_a(self.b); } 0x91 => { self.sub_a(self.c); }
            0x92 => { self.sub_a(self.d); } 0x93 => { self.sub_a(self.e); }
            0x94 => { self.sub_a(self.h); } 0x95 => { self.sub_a(self.l); }
            0x96 => { let v = self.read_byte(memory, self.get_hl()); self.sub_a(v); }
            0x97 => { self.sub_a(self.a); }
            0x98 => { self.sbc_a(self.b); } 0x99 => { self.sbc_a(self.c); }
            0x9A => { self.sbc_a(self.d); } 0x9B => { self.sbc_a(self.e); }
            0x9C => { self.sbc_a(self.h); } 0x9D => { self.sbc_a(self.l); }
            0x9E => { let v = self.read_byte(memory, self.get_hl()); self.sbc_a(v); }
            0x9F => { self.sbc_a(self.a); }
            0xA0 => { self.and_a(self.b); } 0xA1 => { self.and_a(self.c); }
            0xA2 => { self.and_a(self.d); } 0xA3 => { self.and_a(self.e); }
            0xA4 => { self.and_a(self.h); } 0xA5 => { self.and_a(self.l); }
            0xA6 => { let v = self.read_byte(memory, self.get_hl()); self.and_a(v); }
            0xA7 => { self.and_a(self.a); }
            0xA8 => { self.xor_a(self.b); } 0xA9 => { self.xor_a(self.c); }
            0xAA => { self.xor_a(self.d); } 0xAB => { self.xor_a(self.e); }
            0xAC => { self.xor_a(self.h); } 0xAD => { self.xor_a(self.l); }
            0xAE => { let v = self.read_byte(memory, self.get_hl()); self.xor_a(v); }
            0xAF => { self.xor_a(self.a); }
            0xB0 => { self.or_a(self.b); } 0xB1 => { self.or_a(self.c); }
            0xB2 => { self.or_a(self.d); } 0xB3 => { self.or_a(self.e); }
            0xB4 => { self.or_a(self.h); } 0xB5 => { self.or_a(self.l); }
            0xB6 => { let v = self.read_byte(memory, self.get_hl()); self.or_a(v); }
            0xB7 => { self.or_a(self.a); }
            0xB8 => { self.cp_a(self.b); } 0xB9 => { self.cp_a(self.c); }
            0xBA => { self.cp_a(self.d); } 0xBB => { self.cp_a(self.e); }
            0xBC => { self.cp_a(self.h); } 0xBD => { self.cp_a(self.l); }
            0xBE => { let v = self.read_byte(memory, self.get_hl()); self.cp_a(v); }
            0xBF => { self.cp_a(self.a); }

            // Control flow
            0xC0 => { self.internal_cycle(memory); if !self.is_flag_set(ZERO_FLAG) { self.pc = self.pop_stack(memory); self.internal_cycle(memory); } }
            0xC1 => { let v = self.pop_stack(memory); self.set_bc(v); }
            0xC2 => { let addr = self.fetch_word(memory); if !self.is_flag_set(ZERO_FLAG) { self.pc = addr; self.internal_cycle(memory); } }
            0xC3 => { self.pc = self.fetch_word(memory); self.internal_cycle(memory); }
            0xC4 => { let addr = self.fetch_word(memory); if !self.is_flag_set(ZERO_FLAG) { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = addr; } }
            0xC5 => { self.internal_cycle(memory); let v = self.get_bc(); self.push_stack(memory, v); }
            0xC6 => { let v = self.fetch(memory); self.add_a(v); }
            0xC7 => { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = 0x00; }
            0xC8 => { self.internal_cycle(memory); if self.is_flag_set(ZERO_FLAG) { self.pc = self.pop_stack(memory); self.internal_cycle(memory); } }
            0xC9 => { self.pc = self.pop_stack(memory); self.internal_cycle(memory); }
            0xCA => { let addr = self.fetch_word(memory); if self.is_flag_set(ZERO_FLAG) { self.pc = addr; self.internal_cycle(memory); } }
            0xCB => { self.execute_cb(memory); }
            0xCC => { let addr = self.fetch_word(memory); if self.is_flag_set(ZERO_FLAG) { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = addr; } }
            0xCD => { let addr = self.fetch_word(memory); self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = addr; }
            0xCE => { let v = self.fetch(memory); self.adc_a(v); }
            0xCF => { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = 0x08; }

            0xD0 => { self.internal_cycle(memory); if !self.is_flag_set(CARRY_FLAG) { self.pc = self.pop_stack(memory); self.internal_cycle(memory); } }
            0xD1 => { let v = self.pop_stack(memory); self.set_de(v); }
            0xD2 => { let addr = self.fetch_word(memory); if !self.is_flag_set(CARRY_FLAG) { self.pc = addr; self.internal_cycle(memory); } }
            0xD4 => { let addr = self.fetch_word(memory); if !self.is_flag_set(CARRY_FLAG) { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = addr; } }
            0xD5 => { self.internal_cycle(memory); let v = self.get_de(); self.push_stack(memory, v); }
            0xD6 => { let v = self.fetch(memory); self.sub_a(v); }
            0xD7 => { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = 0x10; }
            0xD8 => { self.internal_cycle(memory); if self.is_flag_set(CARRY_FLAG) { self.pc = self.pop_stack(memory); self.internal_cycle(memory); } }
            0xD9 => { self.pc = self.pop_stack(memory); self.internal_cycle(memory); self.ime = true; }
            0xDA => { let addr = self.fetch_word(memory); if self.is_flag_set(CARRY_FLAG) { self.pc = addr; self.internal_cycle(memory); } }
            0xDC => { let addr = self.fetch_word(memory); if self.is_flag_set(CARRY_FLAG) { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = addr; } }
            0xDE => { let v = self.fetch(memory); self.sbc_a(v); }
            0xDF => { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = 0x18; }

            0xE0 => { let n = self.fetch(memory); self.write_byte(memory, 0xFF00 | n as u16, self.a); }
            0xE1 => { let v = self.pop_stack(memory); self.set_hl(v); }
            0xE2 => { self.write_byte(memory, 0xFF00 | self.c as u16, self.a); }
            0xE5 => { self.internal_cycle(memory); let v = self.get_hl(); self.push_stack(memory, v); }
            0xE6 => { let v = self.fetch(memory); self.and_a(v); }
            0xE7 => { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = 0x20; }
            0xE8 => { // ADD SP,n
                let n = self.fetch(memory) as i8 as u16;
                self.f = 0;
                if (self.sp & 0xFF) + (n & 0xFF) > 0xFF { self.set_flag(CARRY_FLAG); }
                if (self.sp & 0xF) + (n & 0xF) > 0xF { self.set_flag(HALF_CARRY_FLAG); }
                self.sp = self.sp.wrapping_add(n);
                self.internal_cycle(memory);
                self.internal_cycle(memory);
            }
            0xE9 => { self.pc = self.get_hl(); }
            0xEA => { let addr = self.fetch_word(memory); self.write_byte(memory, addr, self.a); }
            0xEE => { let v = self.fetch(memory); self.xor_a(v); }
            0xEF => { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = 0x28; }

            0xF0 => { let n = self.fetch(memory); self.a = self.read_byte(memory, 0xFF00 | n as u16); }
            0xF1 => { let v = self.pop_stack(memory); self.set_af(v); }
            0xF2 => { self.a = self.read_byte(memory, 0xFF00 | self.c as u16); }
            0xF3 => { self.ime = false; self.ime_pending = false; } // DI
            0xF5 => { self.internal_cycle(memory); let v = self.get_af(); self.push_stack(memory, v); }
            0xF6 => { let v = self.fetch(memory); self.or_a(v); }
            0xF7 => { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = 0x30; }
            0xF8 => { // LD HL,SP+n
                let n = self.fetch(memory) as i8 as u16;
                self.f = 0;
                if (self.sp & 0xFF) + (n & 0xFF) > 0xFF { self.set_flag(CARRY_FLAG); }
                if (self.sp & 0xF) + (n & 0xF) > 0xF { self.set_flag(HALF_CARRY_FLAG); }
                self.set_hl(self.sp.wrapping_add(n));
                self.internal_cycle(memory);
            }
            0xF9 => { self.sp = self.get_hl(); self.internal_cycle(memory); }
            0xFA => { let addr = self.fetch_word(memory); self.a = self.read_byte(memory, addr); }
            0xFB => { self.ime_pending = true; } // EI (delayed)
            0xFE => { let v = self.fetch(memory); self.cp_a(v); }
            0xFF => { self.internal_cycle(memory); self.push_stack(memory, self.pc); self.pc = 0x38; }

            // Invalid opcodes (no operation on real hardware)
            0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD => {}
        }
    }
}
