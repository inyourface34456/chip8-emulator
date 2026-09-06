#![allow(unused, clippy::cast_possible_truncation)]
use crate::{Cpu, Registers};

impl Cpu {
    /// opcode 00e0
    pub fn cls(&mut self) {
        self.display = [false; Self::DISPLAY_SIZE];
    }

    /// opcode 00ee
    pub fn ret(&mut self) {
        self.sp += 1;
        let ret_adder = self.stack[self.sp as usize];
        self.pc = ret_adder;
        self.stack[self.sp as usize] = 0;
    }

    /// opcode 1nnn where nnn is the address to jump to in hex
    pub fn jmp(&mut self, adder: u16) {
        assert!(
            adder < Self::MEMORY_SIZE.try_into().unwrap_or(4096),
            "tried to jump outside memory bounds, to {adder}"
        );
        self.pc = adder;
    }

    /// opcode 2nnn where nn is the address of the subroutine in hex
    pub fn call(&mut self, adder: u16) {
        assert!(
            adder < Self::MEMORY_SIZE.try_into().unwrap_or(4096),
            "tried to call outside memory bounds, to {adder}"
        );
        self.stack[self.sp as usize] = self.pc;
        self.sp -= 1;
        self.pc = adder;
    }

    /// opcode 3xnn where x is the register and nn is the value to compare to
    pub fn se(&mut self, reg: Registers, val: u8) {
        if self.registers[reg] == val {
            self.pc += 2;
        }
    }

    /// opcode 4xnn where x is the register and nn is the value to compare to
    pub fn sne(&mut self, reg: Registers, val: u8) {
        if self.registers[reg] != val {
            self.pc += 2;
        }
    }

    /// opcode 5xy0 where x and y are registers
    pub fn ser(&mut self, reg_x: Registers, reg_y: Registers) {
        if self.registers[reg_x] == self.registers[reg_y] {
            self.pc += 2;
        }
    }

    /// opcode 6xnn where x is a register and nn is a value
    pub fn ld(&mut self, reg: Registers, val: u8) {
        self.registers[reg] = val;
    }

    /// opcode 7xnn where x is a register and nn is a value
    pub fn add(&mut self, reg: Registers, val: u8) {
        self.registers[reg] = self.registers[reg].wrapping_add(val);
    }

    /// opcode 8xy0 where x and y are registers
    pub fn ldr(&mut self, reg_x: Registers, reg_y: Registers) {
        self.registers[reg_x] = self.registers[reg_y];
    }

    /// opcode 8xy1 where x and y are registers
    pub fn or(&mut self, reg_x: Registers, reg_y: Registers) {
        self.registers[reg_x] |= self.registers[reg_y];
    }

    /// opcode 8xy2 where x and y are registers
    pub fn and(&mut self, reg_x: Registers, reg_y: Registers) {
        self.registers[reg_x] &= self.registers[reg_y];
    }

    /// opcode 8xy3 where x and y are registers
    pub fn xor(&mut self, reg_x: Registers, reg_y: Registers) {
        self.registers[reg_x] ^= self.registers[reg_y];
    }

    /// opcode 8xy4 where you know the drill by now
    pub fn addc(&mut self, reg_x: Registers, reg_y: Registers) {
        let (res, carry) = self.registers[reg_x].carrying_add(self.registers[reg_y], false);
        self.registers[reg_x] = res;
        self.registers[Registers::VF] = u8::from(carry);
    }

    /// opcode 8xy5
    pub fn sub(&mut self, reg_x: Registers, reg_y: Registers) {
        if self.registers[reg_x] >= self.registers[reg_y] {
            self.registers[Registers::VF] = 1;
        } else {
            self.registers[Registers::VF] = 0;
        }

        self.registers[reg_x] = self.registers[reg_x].wrapping_sub(self.registers[reg_y]);
    }

    /// opcode 8xy6
    pub fn shr(&mut self, reg_x: Registers, reg_y: Registers) {
        if Self::SHX_VY_COPIES_TO_VX {
            self.registers[reg_x] = self.registers[reg_y];
        }
        self.registers[0xF] = self.registers[reg_x] & 0x1;
        self.registers[reg_x] /= 2;
    }

    /// opcode 8xy7 (why this one exists rather then calling sub with the regs in reverse order)
    pub fn subn(&mut self, reg_x: Registers, reg_y: Registers) {
        if self.registers[reg_y] >= self.registers[reg_x] {
            self.registers[Registers::VF] = 1;
        } else {
            self.registers[Registers::VF] = 0;
        }

        self.registers[reg_x] = self.registers[reg_y].wrapping_sub(self.registers[reg_x]);
    }

    /// opcode 8xye
    pub fn shl(&mut self, reg_x: Registers, reg_y: Registers) {
        if Self::SHX_VY_COPIES_TO_VX {
            self.registers[reg_x] = self.registers[reg_y];
        }
        self.registers[Registers::VF] = (self.registers[reg_x] & 0x80) >> 7;
        self.registers[reg_x] <<= 1;
    }

    /// opcode 9xy0
    pub fn sner(&mut self, reg_x: Registers, reg_y: Registers) {
        if self.registers[reg_x] != self.registers[reg_y] {
            self.pc += 2;
        }
    }

    /// opcode annn, where nnn is a hex memory address
    pub fn ldi(&mut self, adder: u16) {
        self.index_register = adder;
    }

    /// opcode bnnn
    pub fn ljmp(&mut self, adder: u16) {
        self.pc = u16::from(self.registers[Registers::V0]) + adder;
    }

    /// opcode cxnn
    pub fn rnd(&mut self, reg: Registers, val: u8) {
        let mut output = 0;
        unsafe {
            std::arch::x86_64::_rdrand16_step(&mut output);
        }
        self.registers[reg] = output.to_le_bytes()[0] & val;
    }

    /// opcode dxyn
    pub fn drw(&mut self, reg_x: Registers, reg_y: Registers, n: u8) {
        let xcord = self.registers[reg_x] % Self::DISPLAY_WIDTH as u8;
        let ycord = self.registers[reg_y] % Self::DISPLAY_HEIGHT as u8;
        let mut disabled_px = false;

        for row in 0..n {
            let cy = (ycord + row) % Self::DISPLAY_HEIGHT as u8;
            let bits = self.memory[(self.index_register + u16::from(row)) as usize];

            for col in 0..8 {
                let cx = (xcord + col) % Self::DISPLAY_WIDTH as u8;
                let curr_col =
                    self.display[(usize::from(cy) * Self::DISPLAY_WIDTH) + usize::from(cx)];
                let col = bits & (0x01 << (7 - col));

                if col > 0 {
                    if curr_col {
                        self.display[(usize::from(cy) * Self::DISPLAY_WIDTH) + usize::from(cx)] =
                            false;
                        self.registers[Registers::VF] = 1;
                        disabled_px = true;
                    } else {
                        self.display[(usize::from(cy) * Self::DISPLAY_WIDTH) + usize::from(cx)] =
                            true;
                    }
                }
                if usize::from(cx) == Self::DISPLAY_WIDTH - 1 {
                    break;
                }
            }
            if usize::from(cy) == Self::DISPLAY_HEIGHT - 1 {
                break;
            }
        }
        if !disabled_px {
            self.registers[Registers::VF] = 0;
        }
    }

    /// opcode ex9e
    pub fn skp(&mut self, reg: Registers) {
        if self.keypad[self.registers[reg] as usize] {
            self.pc += 2;
        }
    }

    /// opcode ex9e
    pub fn sknp(&mut self, reg: Registers) {
        if !self.keypad[self.registers[reg] as usize] {
            self.pc += 2;
        }
    }

    /// opcode fx07
    pub fn rdt(&mut self, reg: Registers) {
        self.registers[reg] = *self.delay_timer.lock().unwrap();
    }

    /// opcode fx0a
    pub fn ldwfk(&mut self, reg: Registers) {
        let mut r#break = false;
        loop {
            for (index, key) in self.keypad.iter().enumerate() {
                if *key {
                    self.registers[reg] = index as u8;
                    r#break = true;
                    break;
                }
            }
            if r#break {
                break;
            }
        }
    }

    /// opcode fx15
    pub fn lddt(&mut self, reg: Registers) {
        *self.delay_timer.lock().unwrap() = self.registers[reg];
    }

    /// opcode fx18
    pub fn ldst(&mut self, reg: Registers) {
        *self.sound_timer.lock().unwrap() = self.registers[reg];
    }

    /// opcode fx1e
    pub fn addi(&mut self, reg: Registers) {
        self.index_register += u16::from(self.registers[reg]);
    }

    /// opcode fx29
    pub fn ldf(&mut self, reg: Registers) {
        self.index_register = (u16::from(self.registers[reg]) * 0x05) + Self::FONT_START as u16;
    }

    /// opcode fx33
    pub fn ldb(&mut self, reg: Registers) {
        let reg_val = self.registers[reg];
        let h = reg_val / 100;
        let t = (reg_val - h * 100) / 10;
        let o = reg_val - h * 100 - t * 10;

        self.memory[self.index_register as usize] = h;
        self.memory[self.index_register as usize + 1] = t;
        self.memory[self.index_register as usize + 2] = o;
    }

    /// opcode fx55
    pub fn ldtomem(&mut self, reg: Registers) {
        for regid in 0..=reg.into() {
            self.memory[self.index_register as usize + regid] = self.registers[regid];
        }
    }

    /// opcode fx65
    pub fn ldfrommem(&mut self, reg: Registers) {
        for regid in 0..=reg.into() {
            self.registers[regid] = self.memory[self.index_register as usize + regid];
        }
    }
}
