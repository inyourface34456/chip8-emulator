#![allow(unused, clippy::cast_possible_truncation)]
use crate::Registers;
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};
use std::time::Duration;
use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
struct InstructionInfo {
    nibble1: u8,
    x: u8,
    y: u8,
    n: u8,
    nn: u8,
    nnn: u16
}

impl InstructionInfo {
    fn x_reg(self) -> Registers {
        (self.x as usize).try_into().expect("invalid register")
    }

    fn y_reg(self) -> Registers {
        (self.y as usize).try_into().expect("invalid register")
    }
}

pub struct Cpu {
    pub memory: [u8; Self::MEMORY_SIZE],
    pub registers: [u8; 16],
    pub index_register: u16,
    pub pc: u16,
    pub stack: [u16; Self::STACK_SIZE],
    pub sp: u8,
    pub delay_timer: Arc<Mutex<u8>>,
    pub sound_timer: Arc<Mutex<u8>>,
    pub keypad: [bool; 16],
    pub display: [bool; Self::DISPLAY_SIZE],
}

// stack grows upwards, bottem is index 15, and the top is 0
// sp points to the topmost spot that is ocupied

impl Cpu {
    pub const DISPLAY_SIZE: usize = Self::DISPLAY_WIDTH * Self::DISPLAY_HEIGHT;
    pub const DISPLAY_WIDTH: usize = 64;
    pub const DISPLAY_HEIGHT: usize = 32;
    pub const STACK_SIZE: usize = 17;
    pub const FONT_START: usize = 0;
    pub const MEMORY_SIZE: usize = 4096;
    pub const SHX_VY_COPIES_TO_VX: bool = false;
    pub const DEFAULT_FONT: [u8; 80] = [
        0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
        0x20, 0x60, 0x20, 0x20, 0x70, // 1
        0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
        0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
        0x90, 0x90, 0xF0, 0x10, 0x10, // 4
        0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
        0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
        0xF0, 0x10, 0x20, 0x40, 0x40, // 7
        0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
        0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
        0xF0, 0x90, 0xF0, 0x90, 0x90, // A
        0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
        0xF0, 0x80, 0x80, 0x80, 0xF0, // C
        0xE0, 0x90, 0x90, 0x90, 0xE0, // D
        0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
        0xF0, 0x80, 0xF0, 0x80, 0x80, // F
    ];

    pub fn init() -> Self {
        let mut memory = [0; Self::MEMORY_SIZE];

        for (index, element) in Self::DEFAULT_FONT.iter().enumerate() {
            memory[index + Self::FONT_START] = *element;
        }

        let cpu = Self {
            memory,
            registers: [0u8; 16],
            index_register: 0,
            pc: 0x200,
            stack: [0u16; Self::STACK_SIZE],
            sp: 16,
            delay_timer: Arc::new(Mutex::new(0)),
            sound_timer: Arc::new(Mutex::new(0)),
            keypad: [false; 16],
            display: [false; Self::DISPLAY_SIZE],
        };

        // to decrment the delay timer
        let delay_timer_clone = cpu.delay_timer.clone();
        spawn(move || {
            loop {
                {
                    let mut delay_timer = delay_timer_clone.lock().unwrap();
                    if *delay_timer > 0 {
                        *delay_timer -= 1;
                    }
                }
                sleep(Duration::from_millis(16));
            }
        });

        let sound_timer_clone = cpu.sound_timer.clone();
        spawn(move || {
            loop {
                {
                    let mut sound_timer = sound_timer_clone.lock().unwrap();
                    if *sound_timer > 0 {
                        *sound_timer -= 1;
                        // beeping is not implmented at the moment, dont know an easy way to do it
                    }
                }
                sleep(Duration::from_millis(16));
            }
        });

        cpu
    }

    pub fn load_data(&mut self, data: &[u8]) {
        for (index, byte) in data.iter().enumerate() {
            if index+512 > 4095 {
                break
            }
            self.memory[index+512] = *byte;
        }
    }

    fn fetch_decode(&mut self) -> InstructionInfo {
        let res = (self.memory[self.pc as usize], self.memory[self.pc as usize + 1]);
        self.pc += 2;
        let full_opcode = (u16::from(res.0) << 8) | u16::from(res.1);
        InstructionInfo {
            nibble1: ((full_opcode & 0xf000) >> 12) as u8,
            x: ((full_opcode & 0x0f00) >> 8) as u8,
            y: ((full_opcode & 0x00f0) >> 4) as u8,
            n: (full_opcode & 0x000f) as u8,
            nn: (full_opcode & 0x00ff) as u8,
            nnn: full_opcode & 0x0fff,
        }
    }

    pub fn start(&mut self) {
        while self.pc < 4095 {
            let info = self.fetch_decode();

            match info.nibble1 {
                0 => {
                    match info.nn {
                        0xe0 => self.cls(),
                        0xee => self.ret(),
                        _ => panic!("Unknow opcode: {info:?}")
                    }
                }
                1 => self.jmp(info.nnn),
                2 => self.call(info.nnn),
                3 => self.se(info.x_reg(), info.nn),
                4 => self.sne(info.x_reg(), info.nn),
                5 => self.ser(info.x_reg(), info.y_reg()),
                6 => self.ld(info.x_reg(), info.nn),
                7 => self.add(info.x_reg(), info.nn),
                8 => {
                    match info.n {
                        0 => self.ldr(info.x_reg(), info.y_reg()),
                        1 => self.or(info.x_reg(), info.y_reg()),
                        2 => self.and(info.x_reg(), info.y_reg()),
                        3 => self.xor(info.x_reg(), info.y_reg()),
                        4 => self.addc(info.x_reg(), info.y_reg()),
                        5 => self.sub(info.x_reg(), info.y_reg()),
                        6 => self.shr(info.x_reg(), info.y_reg()),
                        7 => self.subn(info.x_reg(), info.y_reg()),
                        0xe => self.shl(info.x_reg(), info.y_reg()),
                        _ => panic!("Unknown Opcode at {}: {info:?}", self.pc)
                    }
                },
                9 => self.sner(info.x_reg(), info.y_reg()),
                0xa => self.ldi(info.nnn),
                0xb => self.ljmp(info.nnn),
                0xc => self.rnd(info.x_reg(), info.nn),
                0xd => self.drw(info.x_reg(), info.y_reg(), info.n),
                0xe => {
                    match info.nn {
                        0x9e => self.skp(info.x_reg()),
                        0xa1 => self.sknp(info.x_reg()),
                        _ => panic!("Unknown Opcode at {}: {info:?}", self.pc)
                    }
                },
                0xf => {
                    match info.nn {
                        0x07 => self.rdt(info.x_reg()),
                        0x0a => self.ldwfk(info.x_reg()),
                        0x15 => self.lddt(info.x_reg()),
                        0x18 => self.ldst(info.x_reg()),
                        0x1e => self.addi(info.x_reg()),
                        0x29 => self.ldf(info.x_reg()),
                        0x33 => self.ldb(info.x_reg()),
                        0x55 => self.ldtomem(info.x_reg()),
                        0x65 => self.ldfrommem(info.x_reg()),
                        _ => panic!("Unknown Opcode at {:X}: {info:X?}", self.pc)
                    }
                }
                _ => panic!("Unknown Opcode at {}: {info:?}", self.pc)
            }
        }
    }
}

impl Display for Cpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for row in self.display.chunks(Self::DISPLAY_WIDTH) {
            for pixel in row {
                if *pixel {
                    write!(f, "█")?;
                } else {
                    write!(f, " ")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
