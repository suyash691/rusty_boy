use crate::memory::MMU;
use super::{CPU, CARRY_FLAG, HALF_CARRY_FLAG, ZERO_FLAG};

impl CPU {
    pub(super) fn execute_cb(&mut self, memory: &mut MMU) {
        let op = self.fetch(memory);
        let r = op & 0x07;

        match op {
            0x00..=0x07 => { // RLC
                let v = self.get_r(r, memory);
                let result = (v << 1) | (v >> 7);
                self.f = 0;
                if result == 0 { self.set_flag(ZERO_FLAG); }
                if v & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
                self.set_r(r, result, memory);
            }
            0x08..=0x0F => { // RRC
                let v = self.get_r(r, memory);
                let result = (v >> 1) | (v << 7);
                self.f = 0;
                if result == 0 { self.set_flag(ZERO_FLAG); }
                if v & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
                self.set_r(r, result, memory);
            }
            0x10..=0x17 => { // RL
                let v = self.get_r(r, memory);
                let old_c = self.is_flag_set(CARRY_FLAG) as u8;
                let result = (v << 1) | old_c;
                self.f = 0;
                if result == 0 { self.set_flag(ZERO_FLAG); }
                if v & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
                self.set_r(r, result, memory);
            }
            0x18..=0x1F => { // RR
                let v = self.get_r(r, memory);
                let old_c = (self.is_flag_set(CARRY_FLAG) as u8) << 7;
                let result = (v >> 1) | old_c;
                self.f = 0;
                if result == 0 { self.set_flag(ZERO_FLAG); }
                if v & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
                self.set_r(r, result, memory);
            }
            0x20..=0x27 => { // SLA
                let v = self.get_r(r, memory);
                let result = v << 1;
                self.f = 0;
                if result == 0 { self.set_flag(ZERO_FLAG); }
                if v & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
                self.set_r(r, result, memory);
            }
            0x28..=0x2F => { // SRA
                let v = self.get_r(r, memory);
                let result = (v >> 1) | (v & 0x80);
                self.f = 0;
                if result == 0 { self.set_flag(ZERO_FLAG); }
                if v & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
                self.set_r(r, result, memory);
            }
            0x30..=0x37 => { // SWAP
                let v = self.get_r(r, memory);
                let result = (v >> 4) | (v << 4);
                self.f = 0;
                if result == 0 { self.set_flag(ZERO_FLAG); }
                self.set_r(r, result, memory);
            }
            0x38..=0x3F => { // SRL
                let v = self.get_r(r, memory);
                let result = v >> 1;
                self.f = 0;
                if result == 0 { self.set_flag(ZERO_FLAG); }
                if v & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
                self.set_r(r, result, memory);
            }
            0x40..=0x7F => { // BIT
                let bit = (op >> 3) & 7;
                let v = self.get_r(r, memory);
                self.f = (self.f & CARRY_FLAG) | HALF_CARRY_FLAG;
                if v & (1 << bit) == 0 { self.set_flag(ZERO_FLAG); }
            }
            0x80..=0xBF => { // RES
                let bit = (op >> 3) & 7;
                let v = self.get_r(r, memory);
                self.set_r(r, v & !(1 << bit), memory);
            }
            0xC0..=0xFF => { // SET
                let bit = (op >> 3) & 7;
                let v = self.get_r(r, memory);
                self.set_r(r, v | (1 << bit), memory);
            }
        }
    }
}
