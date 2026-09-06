#![allow(unused)]
#![feature(test)]
mod cpu;
mod microcode;
mod register;
mod test;

use cpu::Cpu;
use register::Registers;

fn main() {
    let mut cpu = Cpu::init();

    cpu.ld(Registers::V0, 0);
    cpu.ld(Registers::V1, 0);
    for i in 0..16 {
        cpu.ldi(i * 5);
        cpu.drw(Registers::V0, Registers::V1, 5);
        println!("{cpu}");
        cpu.cls();
    }
}
