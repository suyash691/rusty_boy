use crate::MMU;

pub struct CPU {
    // Registers
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: u8,
    h: u8,
    l: u8,
    sp: u16,
    pc: u16,

    // Clock cycles
    cycles: u32,

    // Interrupt master enable flag
    ime: bool,

    halt: bool,
    stop: bool,
}

// Flag register bits
const ZERO_FLAG: u8 = 0b1000_0000;
const SUBTRACT_FLAG: u8 = 0b0100_0000;
const HALF_CARRY_FLAG: u8 = 0b0010_0000;
const CARRY_FLAG: u8 = 0b0001_0000;

impl CPU {
    pub fn new() -> Self {
        CPU {
            a: 0, b: 0, c: 0, d: 0, e: 0, f: 0, h: 0, l: 0,
            sp: 0, pc: 0, cycles: 0, ime: false,
            halt: false, stop: false,
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stop
    }

    pub fn step(&mut self, memory: &mut MMU) -> u32 {
        let opcode = self.fetch(memory);
        self.execute(opcode, memory)
    }

    fn fetch(&mut self, memory: &MMU) -> u8 {
        let opcode = memory.read_byte(self.pc);
        self.pc = self.pc.wrapping_add(1);
        opcode
    }

    fn execute(&mut self, opcode: u8, memory: &mut MMU) -> u32 {
        match opcode {
            0x00 => self.nop(),
            0x01 => self.ld_bc_nn(memory),
            0x02 => self.ld_bc_a(memory),
            0x03 => self.inc_bc(),
            0x04 => self.inc_b(),
            0x05 => self.dec_b(),
            0x06 => self.ld_b_n(memory),
            0x07 => self.rlca(),
            0x08 => self.ld_nn_sp(memory),
            0x09 => self.add_hl_bc(),
            0x0A => self.ld_a_bc(memory),
            0x0B => self.dec_bc(),
            0x0C => self.inc_c(),
            0x0D => self.dec_c(),
            0x0E => self.ld_c_n(memory),
            0x0F => self.rrca(),
            
            0x10 => self.stop(),
            0x11 => self.ld_de_nn(memory),
            0x12 => self.ld_de_a(memory),
            0x13 => self.inc_de(),
            0x14 => self.inc_d(),
            0x15 => self.dec_d(),
            0x16 => self.ld_d_n(memory),
            0x17 => self.rla(),
            0x18 => self.jr_n(memory),
            0x19 => self.add_hl_de(),
            0x1A => self.ld_a_de(memory),
            0x1B => self.dec_de(),
            0x1C => self.inc_e(),
            0x1D => self.dec_e(),
            0x1E => self.ld_e_n(memory),
            0x1F => self.rra(),
            
            0x20 => self.jr_nz_n(memory),
            0x21 => self.ld_hl_nn(memory),
            0x22 => self.ld_hl_a(memory),
            0x23 => self.inc_hl(),
            0x24 => self.inc_h(),
            0x25 => self.dec_h(),
            0x26 => self.ld_h_n(memory),
            0x27 => self.daa(),
            0x28 => self.jr_z_n(memory),
            0x29 => self.add_hl_hl(),
            0x2A => self.ld_a_hl_inc(memory),
            0x2B => self.dec_hl(),
            0x2C => self.inc_l(),
            0x2D => self.dec_l(),
            0x2E => self.ld_l_n(memory),
            0x2F => self.cpl(),
            
            0x30 => self.jr_nc_n(memory),
            0x31 => self.ld_sp_nn(memory),
            0x32 => self.ld_hl_dec_a(memory),
            0x33 => self.inc_sp(),
            0x34 => self.inc_hl_addr(memory),
            0x35 => self.dec_hl_addr(memory),
            0x36 => self.ld_hl_n(memory),
            0x37 => self.scf(),
            0x38 => self.jr_c_n(memory),
            0x39 => self.add_hl_sp(),
            0x3A => self.ld_a_hl_dec(memory),
            0x3B => self.dec_sp(),
            0x3C => self.inc_a(),
            0x3D => self.dec_a(),
            0x3E => self.ld_a_n(memory),
            0x3F => self.ccf(),
            
            0x40 => self.ld_b_b(),
            0x41 => self.ld_b_c(),
            0x42 => self.ld_b_d(),
            0x43 => self.ld_b_e(),
            0x44 => self.ld_b_h(),
            0x45 => self.ld_b_l(),
            0x46 => self.ld_b_hl(memory),
            0x47 => self.ld_b_a(),
            0x48 => self.ld_c_b(),
            0x49 => self.ld_c_c(),
            0x4A => self.ld_c_d(),
            0x4B => self.ld_c_e(),
            0x4C => self.ld_c_h(),
            0x4D => self.ld_c_l(),
            0x4E => self.ld_c_hl(memory),
            0x4F => self.ld_c_a(),
            
            0x50 => self.ld_d_b(),
            0x51 => self.ld_d_c(),
            0x52 => self.ld_d_d(),
            0x53 => self.ld_d_e(),
            0x54 => self.ld_d_h(),
            0x55 => self.ld_d_l(),
            0x56 => self.ld_d_hl(memory),
            0x57 => self.ld_d_a(),
            0x58 => self.ld_e_b(),
            0x59 => self.ld_e_c(),
            0x5A => self.ld_e_d(),
            0x5B => self.ld_e_e(),
            0x5C => self.ld_e_h(),
            0x5D => self.ld_e_l(),
            0x5E => self.ld_e_hl(memory),
            0x5F => self.ld_e_a(),
            
            0x60 => self.ld_h_b(),
            0x61 => self.ld_h_c(),
            0x62 => self.ld_h_d(),
            0x63 => self.ld_h_e(),
            0x64 => self.ld_h_h(),
            0x65 => self.ld_h_l(),
            0x66 => self.ld_h_hl(memory),
            0x67 => self.ld_h_a(),
            0x68 => self.ld_l_b(),
            0x69 => self.ld_l_c(),
            0x6A => self.ld_l_d(),
            0x6B => self.ld_l_e(),
            0x6C => self.ld_l_h(),
            0x6D => self.ld_l_l(),
            0x6E => self.ld_l_hl(memory),
            0x6F => self.ld_l_a(),
            
            0x70 => self.ld_hl_b(memory),
            0x71 => self.ld_hl_c(memory),
            0x72 => self.ld_hl_d(memory),
            0x73 => self.ld_hl_e(memory),
            0x74 => self.ld_hl_h(memory),
            0x75 => self.ld_hl_l(memory),
            0x76 => self.halt(),
            0x77 => self.ld_hl_a(memory),
            0x78 => self.ld_a_b(),
            0x79 => self.ld_a_c(),
            0x7A => self.ld_a_d(),
            0x7B => self.ld_a_e(),
            0x7C => self.ld_a_h(),
            0x7D => self.ld_a_l(),
            0x7E => self.ld_a_hl(memory),
            0x7F => self.ld_a_a(),
            
            0x80 => self.add_a_b(),
            0x81 => self.add_a_c(),
            0x82 => self.add_a_d(),
            0x83 => self.add_a_e(),
            0x84 => self.add_a_h(),
            0x85 => self.add_a_l(),
            0x86 => self.add_a_hl(memory),
            0x87 => self.add_a_a(),
            0x88 => self.adc_a_b(),
            0x89 => self.adc_a_c(),
            0x8A => self.adc_a_d(),
            0x8B => self.adc_a_e(),
            0x8C => self.adc_a_h(),
            0x8D => self.adc_a_l(),
            0x8E => self.adc_a_hl(memory),
            0x8F => self.adc_a_a(),
            
            0x90 => self.sub_b(),
            0x91 => self.sub_c(),
            0x92 => self.sub_d(),
            0x93 => self.sub_e(),
            0x94 => self.sub_h(),
            0x95 => self.sub_l(),
            0x96 => self.sub_hl(memory),
            //0x97 => self.sub_a(),
            0x98 => self.sbc_a_b(),
            0x99 => self.sbc_a_c(),
            0x9A => self.sbc_a_d(),
            0x9B => self.sbc_a_e(),
            0x9C => self.sbc_a_h(),
            0x9D => self.sbc_a_l(),
            0x9E => self.sbc_a_hl(memory),
            0x9F => self.sbc_a_a(),
            
            0xA0 => self.and_b(),
            0xA1 => self.and_c(),
            0xA2 => self.and_d(),
            0xA3 => self.and_e(),
            0xA4 => self.and_h(),
            0xA5 => self.and_l(),
            0xA6 => self.and_hl(memory),
            //0xA7 => self.and_a(),
            0xA8 => self.xor_b(),
            0xA9 => self.xor_c(),
            0xAA => self.xor_d(),
            0xAB => self.xor_e(),
            0xAC => self.xor_h(),
            0xAD => self.xor_l(),
            0xAE => self.xor_hl(memory),
            0xAF => self.xor_a_self(),
            
            0xB0 => self.or_b(),
            0xB1 => self.or_c(),
            0xB2 => self.or_d(),
            0xB3 => self.or_e(),
            0xB4 => self.or_h(),
            0xB5 => self.or_l(),
            0xB6 => self.or_hl(memory),
            //0xB7 => self.or_a(),
            0xB8 => self.cp_b(),
            0xB9 => self.cp_c(),
            0xBA => self.cp_d(),
            0xBB => self.cp_e(),
            0xBC => self.cp_h(),
            0xBD => self.cp_l(),
            0xBE => self.cp_hl(memory),
            //0xBF => self.cp_a(),
            
            0xC0 => self.ret_nz(memory),
            0xC1 => self.pop_bc(memory),
            0xC2 => self.jp_nz_nn(memory),
            0xC3 => self.jp_nn(memory),
            0xC4 => self.call_nz_nn(memory),
            0xC5 => self.push_bc(memory),
            0xC6 => self.add_a_n(memory),
            0xC7 => self.rst_00h(memory),
            0xC8 => self.ret_z(memory),
            0xC9 => self.ret(memory),
            0xCA => self.jp_z_nn(memory),
            0xCB => self.prefix_cb(memory),
            0xCC => self.call_z_nn(memory),
            0xCD => self.call_nn(memory),
            0xCE => self.adc_a_n(memory),
            0xCF => self.rst_08h(memory),
            
            0xD0 => self.ret_nc(memory),
            0xD1 => self.pop_de(memory),
            0xD2 => self.jp_nc_nn(memory),
            0xD3 => panic!("Invalid opcode: 0xD3"),
            0xD4 => self.call_nc_nn(memory),
            0xD5 => self.push_de(memory),
            0xD6 => self.sub_n(memory),
            0xD7 => self.rst_10h(memory),
            0xD8 => self.ret_c(memory),
            0xD9 => self.reti(memory),
            0xDA => self.jp_c_nn(memory),
            0xDB => panic!("Invalid opcode: 0xDB"),
            0xDC => self.call_c_nn(memory),
            0xDD => panic!("Invalid opcode: 0xDD"),
            0xDE => self.sbc_a_n(memory),
            0xDF => self.rst_18h(memory),
            
            0xE0 => self.ldh_n_a(memory),
            0xE1 => self.pop_hl(memory),
            0xE2 => self.ldh_c_a(memory),
            0xE3 => panic!("Invalid opcode: 0xE3"),
            0xE4 => panic!("Invalid opcode: 0xE4"),
            0xE5 => self.push_hl(memory),
            0xE6 => self.and_n(memory),
            0xE7 => self.rst_20h(memory),
            0xE8 => self.add_sp_n(memory),
            0xE9 => self.jp_hl(),
            0xEA => self.ld_nn_a(memory),
            0xEB => panic!("Invalid opcode: 0xEB"),
            0xEC => panic!("Invalid opcode: 0xEC"),
            0xED => panic!("Invalid opcode: 0xED"),
            0xEE => self.xor_n(memory),
            0xEF => self.rst_28h(memory),
            
            0xF0 => self.ldh_a_n(memory),
            0xF1 => self.pop_af(memory),
            0xF2 => self.ldh_a_c(memory),
            0xF3 => self.di(),
            0xF4 => panic!("Invalid opcode: 0xF4"),
            0xF5 => self.push_af(memory),
            0xF6 => self.or_n(memory),
            0xF7 => self.rst_30h(memory),
            0xF8 => self.ld_hl_sp_n(memory),
            0xF9 => self.ld_sp_hl(),
            0xFA => self.ld_a_nn(memory),
            0xFB => self.ei(),
            0xFC => panic!("Invalid opcode: 0xFC"),
            0xFD => panic!("Invalid opcode: 0xFD"),
            0xFE => self.cp_n(memory),
            0xFF => self.rst_38h(memory),
            
            _ => panic!("Unimplemented opcode: 0x{:02X}", opcode),
        }
    }

    // Helper functions for flag operations
    fn set_flag(&mut self, flag: u8) {
        self.f |= flag;
    }

    fn clear_flag(&mut self, flag: u8) {
        self.f &= !flag;
    }

    fn is_flag_set(&self, flag: u8) -> bool {
        self.f & flag != 0
    }

    // 8-bit load instructions
    fn ld_r_n(&mut self, r: u8, n: u8) -> u32 {
        match r {
            0 => self.b = n,
            1 => self.c = n,
            2 => self.d = n,
            3 => self.e = n,
            4 => self.h = n,
            5 => self.l = n,
            7 => self.a = n,
            _ => panic!("Invalid register index"),
        }
        8
    }

    fn ld_r1_r2(&mut self, r1: u8, r2: u8) -> u32 {
        let value = self.get_register(r2);
        self.set_register(r1, value);
        4 // This instruction takes 4 clock cycles
    }

    fn get_register(&self, r: u8) -> u8 {
        match r {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => self.get_hl() as u8,
            7 => self.a,
            _ => panic!("Invalid register index"),
        }
    }

    // 16-bit load instructions
    fn ld_rr_nn(&mut self, r1: u8, r2: u8, memory: &MMU) -> u32 {
        let low = self.fetch(memory);
        let high = self.fetch(memory);
        self.set_register(r1, high);
        self.set_register(r2, low);
        12
    }

    fn set_register(&mut self, r: u8, value: u8) {
        match r {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 => self.h = value,
            5 => self.l = value,
            6 => self.sp = (self.sp & 0xFF00) | (value as u16),
            7 => self.sp = (self.sp & 0x00FF) | ((value as u16) << 8),
            _ => panic!("Invalid register index"),
        }
    }

    // Arithmetic instructions
    fn add_a(&mut self, n: u8) {
        let (result, carry) = self.a.overflowing_add(n);
        let half_carry = (self.a & 0xF) + (n & 0xF) > 0xF;
        
        self.a = result;
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if half_carry { self.set_flag(HALF_CARRY_FLAG); }
        if carry { self.set_flag(CARRY_FLAG); }
    }

    // Implement CPU instructions

    fn nop(&mut self) -> u32 { 4 }

    fn ld_bc_nn(&mut self, memory: &MMU) -> u32 {
        self.ld_rr_nn(0, 1, memory)
    }

    fn ld_bc_a(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.c, self.b]);
        memory.write_byte(address, self.a);
        8
    }

    fn inc_bc(&mut self) -> u32 {
        let bc = u16::from_le_bytes([self.c, self.b]);
        let result = bc.wrapping_add(1);
        let [c, b] = result.to_le_bytes();
        self.b = b;
        self.c = c;
        8
    }

    fn inc_b(&mut self) -> u32 {
        self.b = self.b.wrapping_add(1);
        self.f &= !SUBTRACT_FLAG;
        if self.b == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.b & 0x0F == 0 { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn dec_b(&mut self) -> u32 {
        self.b = self.b.wrapping_sub(1);
        self.f |= SUBTRACT_FLAG;
        if self.b == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.b & 0x0F == 0x0F { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn ld_b_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory);
        self.ld_r_n(0, n)
    }

    fn ld_nn_sp(&mut self, memory: &mut MMU) -> u32 {
        let low = self.fetch(memory);
        let high = self.fetch(memory);
        let address = u16::from_le_bytes([low, high]);
        let [sp_low, sp_high] = self.sp.to_le_bytes();
        memory.write_byte(address, sp_low);
        memory.write_byte(address.wrapping_add(1), sp_high);
        20
    }

    fn add_hl_bc(&mut self) -> u32 {
        let hl = u16::from_le_bytes([self.l, self.h]);
        let bc = u16::from_le_bytes([self.c, self.b]);
        let (result, carry) = hl.overflowing_add(bc);
        let [l, h] = result.to_le_bytes();
        self.h = h;
        self.l = l;
        self.f &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG);
        if carry { self.set_flag(CARRY_FLAG); }
        if (hl & 0xFFF) + (bc & 0xFFF) > 0xFFF { self.set_flag(HALF_CARRY_FLAG); }
        8
    }

    fn ld_a_bc(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.c, self.b]);
        self.a = memory.read_byte(address);
        8
    }

    fn dec_bc(&mut self) -> u32 {
        let bc = u16::from_le_bytes([self.c, self.b]);
        let result = bc.wrapping_sub(1);
        let [c, b] = result.to_le_bytes();
        self.b = b;
        self.c = c;
        8
    }

    fn inc_c(&mut self) -> u32 {
        self.c = self.c.wrapping_add(1);
        self.f &= !SUBTRACT_FLAG;
        if self.c == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.c & 0x0F == 0 { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn dec_c(&mut self) -> u32 {
        self.c = self.c.wrapping_sub(1);
        self.f |= SUBTRACT_FLAG;
        if self.c == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.c & 0x0F == 0x0F { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn ld_c_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory);
        self.ld_r_n(1, n)
    }

    fn ld_de_nn(&mut self, memory: &MMU) -> u32 {
        self.ld_rr_nn(2, 3, memory)
    }

    fn ld_de_a(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.e, self.d]);
        memory.write_byte(address, self.a);
        8
    }

    fn inc_de(&mut self) -> u32 {
        let de = u16::from_le_bytes([self.e, self.d]);
        let result = de.wrapping_add(1);
        let [e, d] = result.to_le_bytes();
        self.d = d;
        self.e = e;
        8
    }

    fn inc_d(&mut self) -> u32 {
        self.d = self.d.wrapping_add(1);
        self.f &= !SUBTRACT_FLAG;
        if self.d == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.d & 0x0F == 0 { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn dec_d(&mut self) -> u32 {
        self.d = self.d.wrapping_sub(1);
        self.f |= SUBTRACT_FLAG;
        if self.d == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.d & 0x0F == 0x0F { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn ld_d_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory);
        self.ld_r_n(2, n)
    }

    fn jr_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory) as i8;
        self.pc = self.pc.wrapping_add(n as u16);
        12
    }

    fn add_hl_de(&mut self) -> u32 {
        let hl = u16::from_le_bytes([self.l, self.h]);
        let de = u16::from_le_bytes([self.e, self.d]);
        let (result, carry) = hl.overflowing_add(de);
        let [l, h] = result.to_le_bytes();
        self.h = h;
        self.l = l;
        self.f &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG);
        if carry { self.set_flag(CARRY_FLAG); }
        if (hl & 0xFFF) + (de & 0xFFF) > 0xFFF { self.set_flag(HALF_CARRY_FLAG); }
        8
    }

    fn ld_a_de(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.e, self.d]);
        self.a = memory.read_byte(address);
        8
    }

    fn dec_de(&mut self) -> u32 {
        let de = u16::from_le_bytes([self.e, self.d]);
        let result = de.wrapping_sub(1);
        let [e, d] = result.to_le_bytes();
        self.d = d;
        self.e = e;
        8
    }

    fn inc_e(&mut self) -> u32 {
        self.e = self.e.wrapping_add(1);
        self.f &= !SUBTRACT_FLAG;
        if self.e == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.e & 0x0F == 0 { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn dec_e(&mut self) -> u32 {
        self.e = self.e.wrapping_sub(1);
        self.f |= SUBTRACT_FLAG;
        if self.e == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.e & 0x0F == 0x0F { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn ld_e_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory);
        self.ld_r_n(3, n)
    }

    fn jr_nz_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory) as i8;
        if !self.is_flag_set(ZERO_FLAG) {
            self.pc = self.pc.wrapping_add(n as u16);
            12
        } else {
            8
        }
    }

    fn ld_hl_nn(&mut self, memory: &MMU) -> u32 {
        self.ld_rr_nn(4, 5, memory)
    }

    fn ld_hl_a(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        memory.write_byte(address, self.a);
        8
    }

    fn inc_hl(&mut self) -> u32 {
        let hl = u16::from_le_bytes([self.l, self.h]);
        let result = hl.wrapping_add(1);
        let [l, h] = result.to_le_bytes();
        self.h = h;
        self.l = l;
        8
    }

    fn inc_h(&mut self) -> u32 {
        self.h = self.h.wrapping_add(1);
        self.f &= !SUBTRACT_FLAG;
        if self.h == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.h & 0x0F == 0 { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn dec_h(&mut self) -> u32 {
        self.h = self.h.wrapping_sub(1);
        self.f |= SUBTRACT_FLAG;
        if self.h == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.h & 0x0F == 0x0F { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn ld_h_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory);
        self.ld_r_n(4, n)
    }

    fn jr_z_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory) as i8;
        if self.is_flag_set(ZERO_FLAG) {
            self.pc = self.pc.wrapping_add(n as u16);
            12
        } else {
            8
        }
    }

    fn add_hl_hl(&mut self) -> u32 {
        let hl = u16::from_le_bytes([self.l, self.h]);
        let (result, carry) = hl.overflowing_add(hl);
        let [l, h] = result.to_le_bytes();
        self.h = h;
        self.l = l;
        self.f &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG);
        if carry { self.set_flag(CARRY_FLAG); }
        if (hl & 0xFFF) + (hl & 0xFFF) > 0xFFF { self.set_flag(HALF_CARRY_FLAG); }
        8
    }

    fn ld_a_hl_inc(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        self.a = memory.read_byte(address);
        let new_hl = address.wrapping_add(1);
        let [l, h] = new_hl.to_le_bytes();
        self.h = h;
        self.l = l;
        8
    }

    fn dec_hl(&mut self) -> u32 {
        let hl = u16::from_le_bytes([self.l, self.h]);
        let result = hl.wrapping_sub(1);
        let [l, h] = result.to_le_bytes();
        self.h = h;
        self.l = l;
        8
    }

    fn inc_l(&mut self) -> u32 {
        self.l = self.l.wrapping_add(1);
        self.f &= !SUBTRACT_FLAG;
        if self.l == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.l & 0x0F == 0 { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn dec_l(&mut self) -> u32 {
        self.l = self.l.wrapping_sub(1);
        self.f |= SUBTRACT_FLAG;
        if self.l == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.l & 0x0F == 0x0F { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn ld_l_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory);
        self.ld_r_n(5, n)
    }

    fn jr_nc_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory) as i8;
        if !self.is_flag_set(CARRY_FLAG) {
            self.pc = self.pc.wrapping_add(n as u16);
            12
        } else {
            8
        }
    }

    fn ld_sp_nn(&mut self, memory: &MMU) -> u32 {
        let low = self.fetch(memory);
        let high = self.fetch(memory);
        self.sp = u16::from_le_bytes([low, high]);
        12
    }

    fn ld_hl_dec_a(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        memory.write_byte(address, self.a);
        let new_hl = address.wrapping_sub(1);
        let [l, h] = new_hl.to_le_bytes();
        self.h = h;
        self.l = l;
        8
    }

    fn inc_sp(&mut self) -> u32 {
        self.sp = self.sp.wrapping_add(1);
        8
    }

    fn inc_hl_addr(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        let result = value.wrapping_add(1);
        memory.write_byte(address, result);
        self.f &= !SUBTRACT_FLAG;
        if result == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if result & 0x0F == 0 { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        12
    }

    fn dec_hl_addr(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        let result = value.wrapping_sub(1);
        memory.write_byte(address, result);
        self.f |= SUBTRACT_FLAG;
        if result == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if result & 0x0F == 0x0F { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        12
    }

    fn ld_hl_n(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = self.fetch(memory);
        memory.write_byte(address, value);
        12
    }

    fn jr_c_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory) as i8;
        if self.is_flag_set(CARRY_FLAG) {
            self.pc = self.pc.wrapping_add(n as u16);
            12
        } else {
            8
        }
    }

    fn add_hl_sp(&mut self) -> u32 {
        let hl = u16::from_le_bytes([self.l, self.h]);
        let (result, carry) = hl.overflowing_add(self.sp);
        let [l, h] = result.to_le_bytes();
        self.h = h;
        self.l = l;
        self.f &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG);
        if carry { self.set_flag(CARRY_FLAG); }
        if (hl & 0xFFF) + (self.sp & 0xFFF) > 0xFFF { self.set_flag(HALF_CARRY_FLAG); }
        8
    }

    fn ld_a_hl_dec(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        self.a = memory.read_byte(address);
        let new_hl = address.wrapping_sub(1);
        let [l, h] = new_hl.to_le_bytes();
        self.h = h;
        self.l = l;
        8
    }

    fn dec_sp(&mut self) -> u32 {
        self.sp = self.sp.wrapping_sub(1);
        8
    }

    fn inc_a(&mut self) -> u32 {
        self.a = self.a.wrapping_add(1);
        self.f &= !SUBTRACT_FLAG;
        if self.a == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.a & 0x0F == 0 { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn dec_a(&mut self) -> u32 {
        self.a = self.a.wrapping_sub(1);
        self.f |= SUBTRACT_FLAG;
        if self.a == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        if self.a & 0x0F == 0x0F { self.set_flag(HALF_CARRY_FLAG); } else { self.clear_flag(HALF_CARRY_FLAG); }
        4
    }

    fn ld_a_n(&mut self, memory: &MMU) -> u32 {
        self.a = self.fetch(memory);
        8
    }

    fn ld_b_b(&mut self) -> u32 { 4 }
    fn ld_b_c(&mut self) -> u32 {
        self.ld_r1_r2(0, 1); // 0 represents register B, 1 represents register C
        4
    }
    fn ld_b_d(&mut self) -> u32 { self.b = self.d; 4 }
    fn ld_b_e(&mut self) -> u32 { self.b = self.e; 4 }
    fn ld_b_h(&mut self) -> u32 { self.b = self.h; 4 }
    fn ld_b_l(&mut self) -> u32 { self.b = self.l; 4 }
    fn ld_b_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        self.b = memory.read_byte(address);
        8
    }
    fn ld_b_a(&mut self) -> u32 { self.b = self.a; 4 }

    fn ld_c_b(&mut self) -> u32 { self.c = self.b; 4 }
    fn ld_c_c(&mut self) -> u32 { 4 }
    fn ld_c_d(&mut self) -> u32 { self.c = self.d; 4 }
    fn ld_c_e(&mut self) -> u32 { self.c = self.e; 4 }
    fn ld_c_h(&mut self) -> u32 { self.c = self.h; 4 }
    fn ld_c_l(&mut self) -> u32 { self.c = self.l; 4 }
    fn ld_c_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        self.c = memory.read_byte(address);
        8
    }
    fn ld_c_a(&mut self) -> u32 { self.c = self.a; 4 }

    fn ld_d_b(&mut self) -> u32 { self.d = self.b; 4 }
    fn ld_d_c(&mut self) -> u32 { self.d = self.c; 4 }
    fn ld_d_d(&mut self) -> u32 { 4 }
    fn ld_d_e(&mut self) -> u32 {
        self.ld_r1_r2(2, 3); // 2 represents register D, 3 represents register E
        4
    }
    fn ld_d_h(&mut self) -> u32 { self.d = self.h; 4 }
    fn ld_d_l(&mut self) -> u32 { self.d = self.l; 4 }
    fn ld_d_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        self.d = memory.read_byte(address);
        8
    }
    fn ld_d_a(&mut self) -> u32 { self.d = self.a; 4 }

    fn ld_e_b(&mut self) -> u32 { self.e = self.b; 4 }
    fn ld_e_c(&mut self) -> u32 { self.e = self.c; 4 }
    fn ld_e_d(&mut self) -> u32 { self.e = self.d; 4 }
    fn ld_e_e(&mut self) -> u32 { 4 }
    fn ld_e_h(&mut self) -> u32 { self.e = self.h; 4 }
    fn ld_e_l(&mut self) -> u32 { self.e = self.l; 4 }
    fn ld_e_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        self.e = memory.read_byte(address);
        8
    }
    fn ld_e_a(&mut self) -> u32 { self.e = self.a; 4 }

    fn ld_h_b(&mut self) -> u32 { self.h = self.b; 4 }
    fn ld_h_c(&mut self) -> u32 { self.h = self.c; 4 }
    fn ld_h_d(&mut self) -> u32 { self.h = self.d; 4 }
    fn ld_h_e(&mut self) -> u32 { self.h = self.e; 4 }
    fn ld_h_h(&mut self) -> u32 { 4 }
    fn ld_h_l(&mut self) -> u32 { self.h = self.l; 4 }
    fn ld_h_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        self.h = memory.read_byte(address);
        8
    }
    fn ld_h_a(&mut self) -> u32 { self.h = self.a; 4 }

    fn ld_l_b(&mut self) -> u32 { self.l = self.b; 4 }
    fn ld_l_c(&mut self) -> u32 { self.l = self.c; 4 }
    fn ld_l_d(&mut self) -> u32 { self.l = self.d; 4 }
    fn ld_l_e(&mut self) -> u32 { self.l = self.e; 4 }
    fn ld_l_h(&mut self) -> u32 { self.l = self.h; 4 }
    fn ld_l_l(&mut self) -> u32 { 4 }
    fn ld_l_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        self.l = memory.read_byte(address);
        8
    }
    fn ld_l_a(&mut self) -> u32 { self.l = self.a; 4 }

    fn ld_hl_b(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        memory.write_byte(address, self.b);
        8
    }
    fn ld_hl_c(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        memory.write_byte(address, self.c);
        8
    }
    fn ld_hl_d(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        memory.write_byte(address, self.d);
        8
    }
    fn ld_hl_e(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        memory.write_byte(address, self.e);
        8
    }
    fn ld_hl_h(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        memory.write_byte(address, self.h);
        8
    }
    fn ld_hl_l(&mut self, memory: &mut MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        memory.write_byte(address, self.l);
        8
    }

    fn ld_a_b(&mut self) -> u32 { self.a = self.b; 4 }
    fn ld_a_c(&mut self) -> u32 { self.a = self.c; 4 }
    fn ld_a_d(&mut self) -> u32 { self.a = self.d; 4 }
    fn ld_a_e(&mut self) -> u32 { self.a = self.e; 4 }
    fn ld_a_h(&mut self) -> u32 { self.a = self.h; 4 }
    fn ld_a_l(&mut self) -> u32 { self.a = self.l; 4 }
    fn ld_a_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        self.a = memory.read_byte(address);
        8
    }
    fn ld_a_a(&mut self) -> u32 { 4 }

    fn add_a_b(&mut self) -> u32 { self.add_a(self.b); 4 }
    fn add_a_c(&mut self) -> u32 { self.add_a(self.c); 4 }
    fn add_a_d(&mut self) -> u32 { self.add_a(self.d); 4 }
    fn add_a_e(&mut self) -> u32 { self.add_a(self.e); 4 }
    fn add_a_h(&mut self) -> u32 { self.add_a(self.h); 4 }
    fn add_a_l(&mut self) -> u32 { self.add_a(self.l); 4 }
    fn add_a_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        self.add_a(value);
        8
    }
    fn add_a_a(&mut self) -> u32 { self.add_a(self.a); 4 }

    fn adc_a_b(&mut self) -> u32 { self.adc_a(self.b); 4 }
    fn adc_a_c(&mut self) -> u32 { self.adc_a(self.c); 4 }
    fn adc_a_d(&mut self) -> u32 { self.adc_a(self.d); 4 }
    fn adc_a_e(&mut self) -> u32 { self.adc_a(self.e); 4 }
    fn adc_a_h(&mut self) -> u32 { self.adc_a(self.h); 4 }
    fn adc_a_l(&mut self) -> u32 { self.adc_a(self.l); 4 }
    fn adc_a_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        self.adc_a(value);
        8
    }
    fn adc_a_a(&mut self) -> u32 { self.adc_a(self.a); 4 }

    fn sub_b(&mut self) -> u32 { self.sub_a(self.b); 4 }
    fn sub_c(&mut self) -> u32 { self.sub_a(self.c); 4 }
    fn sub_d(&mut self) -> u32 { self.sub_a(self.d); 4 }
    fn sub_e(&mut self) -> u32 { self.sub_a(self.e); 4 }
    fn sub_h(&mut self) -> u32 { self.sub_a(self.h); 4 }
    fn sub_l(&mut self) -> u32 { self.sub_a(self.l); 4 }
    fn sub_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        self.sub_a(value);
        8
    }

    fn sbc_a_b(&mut self) -> u32 { self.sbc_a(self.b); 4 }
    fn sbc_a_c(&mut self) -> u32 { self.sbc_a(self.c); 4 }
    fn sbc_a_d(&mut self) -> u32 { self.sbc_a(self.d); 4 }
    fn sbc_a_e(&mut self) -> u32 { self.sbc_a(self.e); 4 }
    fn sbc_a_h(&mut self) -> u32 { self.sbc_a(self.h); 4 }
    fn sbc_a_l(&mut self) -> u32 { self.sbc_a(self.l); 4 }
    fn sbc_a_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        self.sbc_a(value);
        8
    }
    fn sbc_a_a(&mut self) -> u32 { self.sbc_a(self.a); 4 }

    fn and_b(&mut self) -> u32 { self.and_a(self.b); 4 }
    fn and_c(&mut self) -> u32 { self.and_a(self.c); 4 }
    fn and_d(&mut self) -> u32 { self.and_a(self.d); 4 }
    fn and_e(&mut self) -> u32 { self.and_a(self.e); 4 }
    fn and_h(&mut self) -> u32 { self.and_a(self.h); 4 }
    fn and_l(&mut self) -> u32 { self.and_a(self.l); 4 }
    fn and_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        self.and_a(value);
        8
    }

    fn xor_b(&mut self) -> u32 { self.xor_a(self.b); 4 }
    fn xor_c(&mut self) -> u32 { self.xor_a(self.c); 4 }
    fn xor_d(&mut self) -> u32 { self.xor_a(self.d); 4 }
    fn xor_e(&mut self) -> u32 { self.xor_a(self.e); 4 }
    fn xor_h(&mut self) -> u32 { self.xor_a(self.h); 4 }
    fn xor_l(&mut self) -> u32 { self.xor_a(self.l); 4 }
    fn xor_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        self.xor_a(value);
        8
    }

    fn or_b(&mut self) -> u32 { self.or_a(self.b); 4 }
    fn or_c(&mut self) -> u32 { self.or_a(self.c); 4 }
    fn or_d(&mut self) -> u32 { self.or_a(self.d); 4 }
    fn or_e(&mut self) -> u32 { self.or_a(self.e); 4 }
    fn or_h(&mut self) -> u32 { self.or_a(self.h); 4 }
    fn or_l(&mut self) -> u32 { self.or_a(self.l); 4 }
    fn or_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        self.or_a(value);
        8
    }

    fn cp_b(&mut self) -> u32 { self.cp_a(self.b); 4 }
    fn cp_c(&mut self) -> u32 { self.cp_a(self.c); 4 }
    fn cp_d(&mut self) -> u32 { self.cp_a(self.d); 4 }
    fn cp_e(&mut self) -> u32 { self.cp_a(self.e); 4 }
    fn cp_h(&mut self) -> u32 { self.cp_a(self.h); 4 }
    fn cp_l(&mut self) -> u32 { self.cp_a(self.l); 4 }
    fn cp_hl(&mut self, memory: &MMU) -> u32 {
        let address = u16::from_le_bytes([self.l, self.h]);
        let value = memory.read_byte(address);
        self.cp_a(value);
        8
    }

    fn ret_nz(&mut self, memory: &MMU) -> u32 {
        if !self.is_flag_set(ZERO_FLAG) {
            self.ret(memory);
            20
        } else {
            8
        }
    }

    fn pop_bc(&mut self, memory: &MMU) -> u32 {
        let value = self.pop_stack(memory);
        self.set_bc(value);
        12
    }

    fn jp_nz_nn(&mut self, memory: &MMU) -> u32 {
        let address = self.fetch_word(memory);
        if !self.is_flag_set(ZERO_FLAG) {
            self.pc = address;
            16
        } else {
            12
        }
    }

    fn jp_nn(&mut self, memory: &MMU) -> u32 {
        let address = self.fetch_word(memory);
        self.pc = address;
        16
    }

    fn call_nz_nn(&mut self, memory: &mut MMU) -> u32 {
        let address = self.fetch_word(memory);
        if !self.is_flag_set(ZERO_FLAG) {
            self.push_stack(memory, self.pc);
            self.pc = address;
            24
        } else {
            12
        }
    }

    fn push_bc(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.get_bc());
        16
    }

    fn add_a_n(&mut self, memory: &MMU) -> u32 {
        let value = self.fetch(memory);
        self.add_a(value);
        8
    }

    fn rst_00h(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.pc);
        self.pc = 0x0000;
        16
    }

    fn ret_z(&mut self, memory: &MMU) -> u32 {
        if self.is_flag_set(ZERO_FLAG) {
            self.ret(memory);
            20
        } else {
            8
        }
    }

    fn ret(&mut self, memory: &MMU) -> u32 {
        self.pc = self.pop_stack(memory);
        16
    }

    fn jp_z_nn(&mut self, memory: &MMU) -> u32 {
        let address = self.fetch_word(memory);
        if self.is_flag_set(ZERO_FLAG) {
            self.pc = address;
            16
        } else {
            12
        }
    }

    fn call_z_nn(&mut self, memory: &mut MMU) -> u32 {
        let address = self.fetch_word(memory);
        if self.is_flag_set(ZERO_FLAG) {
            self.push_stack(memory, self.pc);
            self.pc = address;
            24
        } else {
            12
        }
    }

    fn call_nn(&mut self, memory: &mut MMU) -> u32 {
        let address = self.fetch_word(memory);
        self.push_stack(memory, self.pc);
        self.pc = address;
        24
    }

    fn adc_a_n(&mut self, memory: &MMU) -> u32 {
        let value = self.fetch(memory);
        self.adc_a(value);
        8
    }

    fn rst_08h(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.pc);
        self.pc = 0x0008;
        16
    }

    fn ret_nc(&mut self, memory: &MMU) -> u32 {
        if !self.is_flag_set(CARRY_FLAG) {
            self.ret(memory);
            20
        } else {
            8
        }
    }

    fn pop_de(&mut self, memory: &MMU) -> u32 {
        let value = self.pop_stack(memory);
        self.set_de(value);
        12
    }

    fn jp_nc_nn(&mut self, memory: &MMU) -> u32 {
        let address = self.fetch_word(memory);
        if !self.is_flag_set(CARRY_FLAG) {
            self.pc = address;
            16
        } else {
            12
        }
    }

    fn call_nc_nn(&mut self, memory: &mut MMU) -> u32 {
        let address = self.fetch_word(memory);
        if !self.is_flag_set(CARRY_FLAG) {
            self.push_stack(memory, self.pc);
            self.pc = address;
            24
        } else {
            12
        }
    }

    fn push_de(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.get_de());
        16
    }

    fn sub_n(&mut self, memory: &MMU) -> u32 {
        let value = self.fetch(memory);
        self.sub_a(value);
        8
    }

    fn rst_10h(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.pc);
        self.pc = 0x0010;
        16
    }

    fn ret_c(&mut self, memory: &MMU) -> u32 {
        if self.is_flag_set(CARRY_FLAG) {
            self.ret(memory);
            20
        } else {
            8
        }
    }

    fn reti(&mut self, memory: &MMU) -> u32 {
        self.ret(memory);
        self.ime = true;
        16
    }

    fn jp_c_nn(&mut self, memory: &MMU) -> u32 {
        let address = self.fetch_word(memory);
        if self.is_flag_set(CARRY_FLAG) {
            self.pc = address;
            16
        } else {
            12
        }
    }

    fn call_c_nn(&mut self, memory: &mut MMU) -> u32 {
        let address = self.fetch_word(memory);
        if self.is_flag_set(CARRY_FLAG) {
            self.push_stack(memory, self.pc);
            self.pc = address;
            24
        } else {
            12
        }
    }

    fn sbc_a_n(&mut self, memory: &MMU) -> u32 {
        let value = self.fetch(memory);
        self.sbc_a(value);
        8
    }

    fn rst_18h(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.pc);
        self.pc = 0x0018;
        16
    }

    fn ldh_n_a(&mut self, memory: &mut MMU) -> u32 {
        let offset = self.fetch(memory);
        let address = 0xFF00 | (offset as u16);
        memory.write_byte(address, self.a);
        12
    }

    fn pop_hl(&mut self, memory: &MMU) -> u32 {
        let value = self.pop_stack(memory);
        self.set_hl(value);
        12
    }

    fn ldh_c_a(&mut self, memory: &mut MMU) -> u32 {
        let address = 0xFF00 | (self.c as u16);
        memory.write_byte(address, self.a);
        8
    }

    fn push_hl(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.get_hl());
        16
    }

    fn and_n(&mut self, memory: &MMU) -> u32 {
        let value = self.fetch(memory);
        self.and_a(value);
        8
    }

    fn rst_20h(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.pc);
        self.pc = 0x0020;
        16
    }

    fn add_sp_n(&mut self, memory: &MMU) -> u32 {
        let value = self.fetch(memory) as i8 as i16 as u16;
        let (result, carry) = self.sp.overflowing_add(value);
        self.sp = result;

        self.f = 0;
        if carry { self.set_flag(CARRY_FLAG); }
        if (self.sp & 0xF) + (value & 0xF) > 0xF { self.set_flag(HALF_CARRY_FLAG); }

        16
    }

    fn jp_hl(&mut self) -> u32 {
        self.pc = self.get_hl();
        4
    }

    fn ld_nn_a(&mut self, memory: &mut MMU) -> u32 {
        let address = self.fetch_word(memory);
        memory.write_byte(address, self.a);
        16
    }

    fn xor_n(&mut self, memory: &MMU) -> u32 {
        let value = self.fetch(memory);
        self.xor_a(value);
        8
    }

    fn rst_28h(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.pc);
        self.pc = 0x0028;
        16
    }

    fn ldh_a_n(&mut self, memory: &MMU) -> u32 {
        let offset = self.fetch(memory);
        let address = 0xFF00 | (offset as u16);
        self.a = memory.read_byte(address);
        12
    }

    fn pop_af(&mut self, memory: &MMU) -> u32 {
        let value = self.pop_stack(memory);
        self.set_af(value);
        12
    }

    fn ldh_a_c(&mut self, memory: &MMU) -> u32 {
        let address = 0xFF00 | (self.c as u16);
        self.a = memory.read_byte(address);
        8
    }

    fn di(&mut self) -> u32 {
        self.ime = false;
        4
    }

    fn push_af(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.get_af());
        16
    }

    fn or_n(&mut self, memory: &MMU) -> u32 {
        let value = self.fetch(memory);
        self.or_a(value);
        8
    }

    fn rst_30h(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.pc);
        self.pc = 0x0030;
        16
    }

    fn ld_hl_sp_n(&mut self, memory: &MMU) -> u32 {
        let n = self.fetch(memory) as i8 as i16;
        let (result, carry) = self.sp.overflowing_add(n as u16);
        self.set_hl(result);

        self.f = 0;
        if carry { self.set_flag(CARRY_FLAG); }
        if (self.sp & 0xF) + (n as u16 & 0xF) > 0xF { self.set_flag(HALF_CARRY_FLAG); }

        12
    }

    fn ld_sp_hl(&mut self) -> u32 {
        self.sp = self.get_hl();
        8
    }

    fn ld_a_nn(&mut self, memory: &MMU) -> u32 {
        let address = self.fetch_word(memory);
        self.a = memory.read_byte(address);
        16
    }

    fn ei(&mut self) -> u32 {
        self.ime = true;
        4
    }

    fn cp_n(&mut self, memory: &MMU) -> u32 {
        let value = self.fetch(memory);
        self.cp_a(value);
        8
    }

    fn rst_38h(&mut self, memory: &mut MMU) -> u32 {
        self.push_stack(memory, self.pc);
        self.pc = 0x0038;
        16
    }

    // Helper methods

    fn fetch_word(&mut self, memory: &MMU) -> u16 {
        let low = self.fetch(memory);
        let high = self.fetch(memory);
        u16::from_le_bytes([low, high])
    }

    fn push_stack(&mut self, memory: &mut MMU, value: u16) {
        self.sp = self.sp.wrapping_sub(2);
        let [low, high] = value.to_le_bytes();
        memory.write_byte(self.sp, low);
        memory.write_byte(self.sp.wrapping_add(1), high);
    }

    fn pop_stack(&mut self, memory: &MMU) -> u16 {
        let low = memory.read_byte(self.sp);
        let high = memory.read_byte(self.sp.wrapping_add(1));
        self.sp = self.sp.wrapping_add(2);
        u16::from_le_bytes([low, high])
    }

    // Arithmetic and logical operations

    fn adc_a(&mut self, n: u8) {
        let carry = if self.is_flag_set(CARRY_FLAG) { 1 } else { 0 };
        let (result, carry1) = self.a.overflowing_add(n);
        let (result, carry2) = result.overflowing_add(carry);
        let half_carry = (self.a & 0xF) + (n & 0xF) + carry > 0xF;

        self.a = result;
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if half_carry { self.set_flag(HALF_CARRY_FLAG); }
        if carry1 || carry2 { self.set_flag(CARRY_FLAG); }
    }

    fn sub_a(&mut self, n: u8) {
        let (result, carry) = self.a.overflowing_sub(n);
        let half_carry = (self.a & 0xF) < (n & 0xF);

        self.a = result;
        self.f = SUBTRACT_FLAG;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if half_carry { self.set_flag(HALF_CARRY_FLAG); }
        if carry { self.set_flag(CARRY_FLAG); }
    }

    fn sbc_a(&mut self, n: u8) {
        let carry = if self.is_flag_set(CARRY_FLAG) { 1 } else { 0 };
        let (result, carry1) = self.a.overflowing_sub(n);
        let (result, carry2) = result.overflowing_sub(carry);
        let half_carry = (self.a & 0xF) < (n & 0xF) + carry;

        self.a = result;
        self.f = SUBTRACT_FLAG;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if half_carry { self.set_flag(HALF_CARRY_FLAG); }
        if carry1 || carry2 { self.set_flag(CARRY_FLAG); }
    }

    fn and_a(&mut self, n: u8) {
        self.a &= n;
        self.f = HALF_CARRY_FLAG;
        if self.a == 0 { self.set_flag(ZERO_FLAG); }
    }

    fn xor_a(&mut self, n: u8) {
        self.a ^= n;
        self.f = 0;
        if self.a == 0 { self.set_flag(ZERO_FLAG); }
    }

    fn xor_a_self(&mut self) -> u32 {
        self.xor_a(self.a);
        4
    }

    fn or_a(&mut self, n: u8) {
        self.a |= n;
        self.f = 0;
        if self.a == 0 { self.set_flag(ZERO_FLAG); }
    }

    fn cp_a(&mut self, n: u8) {
        let result = self.a.wrapping_sub(n);
        self.f = SUBTRACT_FLAG;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if (self.a & 0xF) < (n & 0xF) { self.set_flag(HALF_CARRY_FLAG); }
        if self.a < n { self.set_flag(CARRY_FLAG); }
    }

    // 16-bit register operations

    fn get_bc(&self) -> u16 {
        u16::from_le_bytes([self.c, self.b])
    }

    fn set_bc(&mut self, value: u16) {
        let [c, b] = value.to_le_bytes();
        self.b = b;
        self.c = c;
    }

    fn get_de(&self) -> u16 {
        u16::from_le_bytes([self.e, self.d])
    }

    fn set_de(&mut self, value: u16) {
        let [e, d] = value.to_le_bytes();
        self.d = d;
        self.e = e;
    }

    fn get_hl(&self) -> u16 {
        u16::from_le_bytes([self.l, self.h])
    }

    fn set_hl(&mut self, value: u16) {
        let [l, h] = value.to_le_bytes();
        self.h = h;
        self.l = l;
    }

    fn get_af(&self) -> u16 {
        u16::from_le_bytes([self.f, self.a])
    }

    fn set_af(&mut self, value: u16) {
        let [f, a] = value.to_le_bytes();
        self.a = a;
        self.f = f & 0xF0; // Only upper 4 bits of F are used
    }

    // CB-prefixed instructions

    fn execute_cb(&mut self, memory: &mut MMU) -> u32 {
        let opcode = self.fetch(memory);
        match opcode {
            0x00..=0x07 => self.rlc_r(opcode & 0x07, memory),
            0x08..=0x0F => self.rrc_r(opcode & 0x07, memory),
            0x10..=0x17 => self.rl_r(opcode & 0x07, memory),
            0x18..=0x1F => self.rr_r(opcode & 0x07, memory),
            0x20..=0x27 => self.sla_r(opcode & 0x07, memory),
            0x28..=0x2F => self.sra_r(opcode & 0x07, memory),
            0x30..=0x37 => self.swap_r(opcode & 0x07, memory),
            0x38..=0x3F => self.srl_r(opcode & 0x07, memory),
            0x40..=0x7F => self.bit_b_r(opcode, memory),
            0x80..=0xBF => self.res_b_r(opcode, memory),
            0xC0..=0xFF => self.set_b_r(opcode, memory),
        }
    }

    fn rlc_r(&mut self, r: u8, memory: &mut MMU) -> u32 {
        let value = self.get_r(r, memory);
        let result = (value << 1) | (value >> 7);
        self.set_r(r, result, memory);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
        8
    }

    fn rrc_r(&mut self, r: u8, memory: &mut MMU) -> u32 {
        let value = self.get_r(r, memory);
        let result = (value >> 1) | (value << 7);
        self.set_r(r, result, memory);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        8
    }

    fn rl_r(&mut self, r: u8, memory: &mut MMU) -> u32 {
        let value = self.get_r(r, memory);
        let old_carry = if self.is_flag_set(CARRY_FLAG) { 1 } else { 0 };
        let result = (value << 1) | old_carry;
        self.set_r(r, result, memory);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
        8
    }

    fn rr_r(&mut self, r: u8, memory: &mut MMU) -> u32 {
        let value = self.get_r(r, memory);
        let old_carry = if self.is_flag_set(CARRY_FLAG) { 0x80 } else { 0 };
        let result = (value >> 1) | old_carry;
        self.set_r(r, result, memory);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        8
    }

    fn sla_r(&mut self, r: u8, memory: &mut MMU) -> u32 {
        let value = self.get_r(r, memory);
        let result = value << 1;
        self.set_r(r, result, memory);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
        8
    }

    fn sra_r(&mut self, r: u8, memory: &mut MMU) -> u32 {
        let value = self.get_r(r, memory);
        let result = (value >> 1) | (value & 0x80);
        self.set_r(r, result, memory);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        8
    }

    fn swap_r(&mut self, r: u8, memory: &mut MMU) -> u32 {
        let value = self.get_r(r, memory);
        let result = ((value & 0xF) << 4) | ((value & 0xF0) >> 4);
        self.set_r(r, result, memory);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        8
    }

    fn srl_r(&mut self, r: u8, memory: &mut MMU) -> u32 {
        let value = self.get_r(r, memory);
        let result = value >> 1;
        self.set_r(r, result, memory);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        8
    }

    fn bit_b_r(&mut self, opcode: u8, memory: &MMU) -> u32 {
        let b = (opcode >> 3) & 0x07;
        let r = opcode & 0x07;
        let value = self.get_r(r, memory);
        let result = value & (1 << b);
        self.f &= !ZERO_FLAG;
        self.f |= HALF_CARRY_FLAG;
        self.f &= !SUBTRACT_FLAG;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        8
    }

    fn res_b_r(&mut self, opcode: u8, memory: &mut MMU) -> u32 {
        let b = (opcode >> 3) & 0x07;
        let r = opcode & 0x07;
        let value = self.get_r(r, memory);
        let result = value & !(1 << b);
        self.set_r(r, result, memory);
        8
    }

    fn set_b_r(&mut self, opcode: u8, memory: &mut MMU) -> u32 {
        let b = (opcode >> 3) & 0x07;
        let r = opcode & 0x07;
        let value = self.get_r(r, memory);
        let result = value | (1 << b);
        self.set_r(r, result, memory);
        8
    }

    fn get_r(&self, r: u8, memory: &MMU) -> u8 {
        match r {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => memory.read_byte(self.get_hl()),
            7 => self.a,
            _ => panic!("Invalid register index"),
        }
    }

    fn set_r(&mut self, r: u8, value: u8, memory: &mut MMU) {
        match r {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 => self.h = value,
            5 => self.l = value,
            6 => {
                let address = self.get_hl();
                memory.write_byte(address, value);
            },
            7 => self.a = value,
            _ => panic!("Invalid register index"),
        }
    }

    // Interrupt handling
    pub fn handle_interrupts(&mut self, memory: &mut MMU, interrupts: u8) -> bool {
        if !self.ime {
            return false;
        }

        let ie = memory.read_byte(0xFFFF);
        let if_ = memory.read_byte(0xFF0F);
        let interrupts = ie & if_ & interrupts;

        if interrupts == 0 {
            return false;
        }

        self.ime = false;
        self.halt = false;

        for i in 0..5 {
            if interrupts & (1 << i) != 0 {
                // Clear the interrupt flag
                memory.write_byte(0xFF0F, if_ & !(1 << i));

                // Push the current PC onto the stack
                self.push_stack(memory, self.pc);

                // Jump to the interrupt vector
                self.pc = match i {
                    0 => 0x0040, // V-Blank
                    1 => 0x0048, // LCD STAT
                    2 => 0x0050, // Timer
                    3 => 0x0058, // Serial
                    4 => 0x0060, // Joypad
                    _ => unreachable!(),
                };

                return true;
            }
        }

        false
    }

    // HALT and STOP instructions
    fn halt(&mut self) -> u32 {
        self.halt = true;
        4
    }

    fn stop(&mut self) -> u32 {
        self.stop = true;
        4
    }

    // Additional helper methods for common operations
    fn add_hl(&mut self, value: u16) {
        let hl = self.get_hl();
        let (result, carry) = hl.overflowing_add(value);
        self.set_hl(result);

        self.f &= !(SUBTRACT_FLAG | CARRY_FLAG | HALF_CARRY_FLAG);
        if carry { self.set_flag(CARRY_FLAG); }
        if (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF { self.set_flag(HALF_CARRY_FLAG); }
    }

    fn jr(&mut self, offset: i8) {
        self.pc = self.pc.wrapping_add(offset as u16);
    }

    // Instruction implementations for remaining opcodes
    fn rlca(&mut self) -> u32 {
        let a = self.a;
        self.a = a.rotate_left(1);
        self.f = 0;
        if self.a & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        4
    }

    fn rrca(&mut self) -> u32 {
        let a = self.a;
        self.a = a.rotate_right(1);
        self.f = 0;
        if self.a & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
        4
    }

    fn rla(&mut self) -> u32 {
        let a = self.a;
        let old_carry = self.is_flag_set(CARRY_FLAG) as u8;
        self.a = (a << 1) | old_carry;
        self.f = 0;
        if a & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
        4
    }

    fn rra(&mut self) -> u32 {
        let a = self.a;
        let old_carry = (self.is_flag_set(CARRY_FLAG) as u8) << 7;
        self.a = (a >> 1) | old_carry;
        self.f = 0;
        if a & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        4
    }

    fn daa(&mut self) -> u32 {
        let mut a = self.a;
        let mut adjust = 0;

        if self.is_flag_set(HALF_CARRY_FLAG) || (!self.is_flag_set(SUBTRACT_FLAG) && (a & 0x0F) > 9) {
            adjust |= 0x06;
        }

        if self.is_flag_set(CARRY_FLAG) || (!self.is_flag_set(SUBTRACT_FLAG) && a > 0x99) {
            adjust |= 0x60;
            self.set_flag(CARRY_FLAG);
        } else {
            self.clear_flag(CARRY_FLAG);
        }

        if self.is_flag_set(SUBTRACT_FLAG) {
            a = a.wrapping_sub(adjust);
        } else {
            a = a.wrapping_add(adjust);
        }

        self.a = a;
        self.clear_flag(HALF_CARRY_FLAG);
        if a == 0 { self.set_flag(ZERO_FLAG); } else { self.clear_flag(ZERO_FLAG); }
        4
    }

    fn cpl(&mut self) -> u32 {
        self.a = !self.a;
        self.set_flag(SUBTRACT_FLAG);
        self.set_flag(HALF_CARRY_FLAG);
        4
    }

    fn scf(&mut self) -> u32 {
        self.clear_flag(SUBTRACT_FLAG);
        self.clear_flag(HALF_CARRY_FLAG);
        self.set_flag(CARRY_FLAG);
        4
    }

    fn ccf(&mut self) -> u32 {
        self.clear_flag(SUBTRACT_FLAG);
        self.clear_flag(HALF_CARRY_FLAG);
        if self.is_flag_set(CARRY_FLAG) {
            self.clear_flag(CARRY_FLAG);
        } else {
            self.set_flag(CARRY_FLAG);
        }
        4
    }

    // Helper method for prefix CB instructions
    fn prefix_cb(&mut self, memory: &mut MMU) -> u32 {
        let opcode = self.fetch(memory);
        match opcode {
            0x00..=0x3F => self.rotate_shift(opcode, memory),
            0x40..=0x7F => self.bit(opcode, memory),
            0x80..=0xBF => self.res(opcode, memory),
            0xC0..=0xFF => self.set(opcode, memory),
        }
    }

    fn rotate_shift(&mut self, opcode: u8, memory: &mut MMU) -> u32 {
        let r = opcode & 0x07;
        let value = self.get_r(r, memory);
        let result = match opcode & 0xF8 {
            0x00 => self.rlc(value),
            0x08 => self.rrc(value),
            0x10 => self.rl(value),
            0x18 => self.rr(value),
            0x20 => self.sla(value),
            0x28 => self.sra(value),
            0x30 => self.swap(value),
            0x38 => self.srl(value),
            _ => unreachable!(),
        };
        self.set_r(r, result, memory);
        8
    }

    fn bit(&mut self, opcode: u8, memory: &MMU) -> u32 {
        let b = (opcode >> 3) & 0x07;
        let r = opcode & 0x07;
        let value = self.get_r(r, memory);
        self.clear_flag(SUBTRACT_FLAG);
        self.set_flag(HALF_CARRY_FLAG);
        if value & (1 << b) == 0 {
            self.set_flag(ZERO_FLAG);
        } else {
            self.clear_flag(ZERO_FLAG);
        }
        8
    }

    fn res(&mut self, opcode: u8, memory: &mut MMU) -> u32 {
        let b = (opcode >> 3) & 0x07;
        let r = opcode & 0x07;
        let value = self.get_r(r, memory);
        let result = value & !(1 << b);
        self.set_r(r, result, memory);
        8
    }

    fn set(&mut self, opcode: u8, memory: &mut MMU) -> u32 {
        let b = (opcode >> 3) & 0x07;
        let r = opcode & 0x07;
        let value = self.get_r(r, memory);
        let result = value | (1 << b);
        self.set_r(r, result, memory);
        8
    }

    // Helper methods for rotate and shift operations
    fn rlc(&mut self, value: u8) -> u8 {
        let result = value.rotate_left(1);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
        result
    }

    fn rrc(&mut self, value: u8) -> u8 {
        let result = value.rotate_right(1);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        result
    }

    fn rl(&mut self, value: u8) -> u8 {
        let carry = self.is_flag_set(CARRY_FLAG) as u8;
        let result = (value << 1) | carry;
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
        result
    }

    fn rr(&mut self, value: u8) -> u8 {
        let carry = (self.is_flag_set(CARRY_FLAG) as u8) << 7;
        let result = (value >> 1) | carry;
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        result
    }

    fn sla(&mut self, value: u8) -> u8 {
        let result = value << 1;
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x80 != 0 { self.set_flag(CARRY_FLAG); }
        result
    }

    fn sra(&mut self, value: u8) -> u8 {
        let result = (value >> 1) | (value & 0x80);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        result
    }

    fn swap(&mut self, value: u8) -> u8 {
        let result = ((value & 0xF0) >> 4) | ((value & 0x0F) << 4);
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        result
    }

    fn srl(&mut self, value: u8) -> u8 {
        let result = value >> 1;
        self.f = 0;
        if result == 0 { self.set_flag(ZERO_FLAG); }
        if value & 0x01 != 0 { self.set_flag(CARRY_FLAG); }
        result
    }
}