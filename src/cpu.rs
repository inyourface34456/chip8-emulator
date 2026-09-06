#![allow(unused, clippy::cast_possible_truncation)]
use crate::Registers;
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};
use std::time::Duration;

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

    pub fn init() -> Self {
        let mut memory = [0; Self::MEMORY_SIZE];
        let default_font = [
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

        for (index, element) in default_font.iter().enumerate() {
            memory[index] = *element;
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
}

impl std::fmt::Display for Cpu {
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
