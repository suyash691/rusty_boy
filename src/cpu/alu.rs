use super::{CPU, CARRY_FLAG, HALF_CARRY_FLAG, SUBTRACT_FLAG, ZERO_FLAG};
#[allow(unused_imports)]
use crate::memory::MMU;

impl CPU {
    pub(super) fn add_a(&mut self, n: u8) {
        let (result, carry) = self.a.overflowing_add(n);
        let hc = (self.a & 0xF) + (n & 0xF) > 0xF;
        self.a = result;
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if hc { self.set_flag(HALF_CARRY_FLAG); }
        if carry { self.set_flag(CARRY_FLAG); }
    }

    pub(super) fn adc_a(&mut self, n: u8) {
        let c = if self.is_flag_set(CARRY_FLAG) { 1u16 } else { 0 };
        let full = self.a as u16 + n as u16 + c;
        let hc = (self.a & 0xF) + (n & 0xF) + c as u8 > 0xF;
        self.a = full as u8;
        self.f = 0;
        if self.a == 0 { self.set_flag(ZERO_FLAG); }
        if hc { self.set_flag(HALF_CARRY_FLAG); }
        if full > 0xFF { self.set_flag(CARRY_FLAG); }
    }

    pub(super) fn sub_a(&mut self, n: u8) {
        let (result, carry) = self.a.overflowing_sub(n);
        let hc = (self.a & 0xF) < (n & 0xF);
        self.a = result;
        self.f = SUBTRACT_FLAG;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if hc { self.set_flag(HALF_CARRY_FLAG); }
        if carry { self.set_flag(CARRY_FLAG); }
    }

    pub(super) fn sbc_a(&mut self, n: u8) {
        let c = if self.is_flag_set(CARRY_FLAG) { 1u16 } else { 0 };
        let full = (self.a as u16).wrapping_sub(n as u16).wrapping_sub(c);
        let hc = (self.a & 0xF) < (n & 0xF) + c as u8;
        self.a = full as u8;
        self.f = SUBTRACT_FLAG;
        if self.a == 0 { self.set_flag(ZERO_FLAG); }
        if hc { self.set_flag(HALF_CARRY_FLAG); }
        if full > 0xFF { self.set_flag(CARRY_FLAG); }
    }

    pub(super) fn and_a(&mut self, n: u8) {
        self.a &= n;
        self.f = HALF_CARRY_FLAG;
        if self.a == 0 { self.set_flag(ZERO_FLAG); }
    }

    pub(super) fn xor_a(&mut self, n: u8) {
        self.a ^= n;
        self.f = 0;
        if self.a == 0 { self.set_flag(ZERO_FLAG); }
    }

    pub(super) fn or_a(&mut self, n: u8) {
        self.a |= n;
        self.f = 0;
        if self.a == 0 { self.set_flag(ZERO_FLAG); }
    }

    pub(super) fn cp_a(&mut self, n: u8) {
        let result = self.a.wrapping_sub(n);
        self.f = SUBTRACT_FLAG;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if (self.a & 0xF) < (n & 0xF) { self.set_flag(HALF_CARRY_FLAG); }
        if self.a < n { self.set_flag(CARRY_FLAG); }
    }

    pub(super) fn inc(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        self.f = (self.f & CARRY_FLAG)
            | if r == 0 { ZERO_FLAG } else { 0 }
            | if r & 0xF == 0 { HALF_CARRY_FLAG } else { 0 };
        r
    }

    pub(super) fn dec(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        self.f = (self.f & CARRY_FLAG) | SUBTRACT_FLAG
            | if r == 0 { ZERO_FLAG } else { 0 }
            | if r & 0xF == 0xF { HALF_CARRY_FLAG } else { 0 };
        r
    }

    pub(super) fn add_hl(&mut self, v: u16) {
        let hl = self.get_hl();
        let (result, carry) = hl.overflowing_add(v);
        self.set_hl(result);
        self.f &= ZERO_FLAG;
        if (hl & 0xFFF) + (v & 0xFFF) > 0xFFF { self.set_flag(HALF_CARRY_FLAG); }
        if carry { self.set_flag(CARRY_FLAG); }
    }

    pub(super) fn daa(&mut self) {
        let mut a = self.a;
        let mut adjust = 0u8;
        if self.is_flag_set(HALF_CARRY_FLAG) || (!self.is_flag_set(SUBTRACT_FLAG) && (a & 0xF) > 9) {
            adjust |= 0x06;
        }
        if self.is_flag_set(CARRY_FLAG) || (!self.is_flag_set(SUBTRACT_FLAG) && a > 0x99) {
            adjust |= 0x60;
            self.set_flag(CARRY_FLAG);
        }
        if self.is_flag_set(SUBTRACT_FLAG) {
            a = a.wrapping_sub(adjust);
        } else {
            a = a.wrapping_add(adjust);
        }
        self.a = a;
        self.clear_flag(HALF_CARRY_FLAG);
        if a == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
    }
}
