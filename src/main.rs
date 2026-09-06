#![allow(unused)]
#![feature(test)]
mod cpu;
mod microcode;
mod register;
mod test;

use cpu::Cpu;
use register::Registers;
use std::{env::{Args, args}, io::Read};

fn main() {
    let args: Vec<String> = args().collect();
    if args.len() != 2 {
        println!("Usage: {} [filename.ch8]", args[0]);
        std::process::exit(1)
    }
    let mut file_data = Vec::new();
    let mut file = std::fs::File::open(&args[1]).expect("file does not exist");
    file.read_to_end(&mut file_data);
    let mut cpu = Cpu::init();
    cpu.load_data(&file_data);
    cpu.start();
}
