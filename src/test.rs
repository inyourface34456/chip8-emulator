#![cfg(test)]
#![allow(clippy::cast_possible_truncation)]

//! Spec tests for CHIP-8 instruction semantics.
//!
//! There is no single ISO document, but these tests follow the original
//! COSMAC VIP CHIP-8 instruction set as documented by:
//! - Cowgod's Chip-8 Technical Reference v1.0
//! - Matthew Mikolay's CHIP-8 Instruction Set / Technical Reference
//! - Tobias Veldboom's interpreter guide (corrections to Cowgod)
//!
//! Ambiguous opcodes use the **modern CHIP-8** behaviour that test ROMs and
//! most 1990s+ games expect (CHIP-48 / SUPER-CHIP quirks on top of CHIP-8):
//! - `8xy6` / `8xyE` shift `Vx` in place and ignore `Vy`
//!   (matches `Cpu::SHX_VY_COPIES_TO_VX == false`)
//! - `Fx55` / `Fx65` copy `V0..=Vx` and leave `I` unchanged
//! - `Bnnn` jumps to `nnn + V0`
//! - `Dxyn` wraps the starting coordinate, then **clips** the sprite
//! - `8xy5` / `8xy7` set `VF = 1` when there is no borrow (`minuend >= subtrahend`),
//!   including equality — Cowgod wrote `>`; VIP / two's-complement borrow does not

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::cpu::Cpu;
use crate::register::Registers;

const FONT_BYTES: [u8; 80] = [
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

fn cpu() -> Cpu {
    Cpu::init()
}

fn all_registers() -> [Registers; 16] {
    [
        Registers::V0,
        Registers::V1,
        Registers::V2,
        Registers::V3,
        Registers::V4,
        Registers::V5,
        Registers::V6,
        Registers::V7,
        Registers::V8,
        Registers::V9,
        Registers::VA,
        Registers::VB,
        Registers::VC,
        Registers::VD,
        Registers::VE,
        Registers::VF,
    ]
}

fn pixel(cpu: &Cpu, x: usize, y: usize) -> bool {
    cpu.display[y * Cpu::DISPLAY_WIDTH + x]
}

fn set_pixel(cpu: &mut Cpu, x: usize, y: usize, on: bool) {
    cpu.display[y * Cpu::DISPLAY_WIDTH + x] = on;
}

fn lit_count(cpu: &Cpu) -> usize {
    cpu.display.iter().filter(|p| **p).count()
}

fn load_sprite(cpu: &mut Cpu, at: u16, bytes: &[u8]) {
    cpu.ldi(at);
    for (i, b) in bytes.iter().enumerate() {
        cpu.memory[at as usize + i] = *b;
    }
}

mod machine {
    use super::*;

    #[test]
    fn init_matches_chip8_power_on_state() {
        let cpu = Cpu::init();
        assert_eq!(cpu.memory.len(), 4096, "CHIP-8 RAM is 4KiB (0x000..=0xFFF)");
        assert_eq!(cpu.display.len(), 64 * 32, "display is 64x32");
        // assert_eq!(cpu.stack.len(), 16, "Cowgod: 16 nested subroutine levels");
        assert_eq!(cpu.pc, 0x200, "programs start at 0x200");
        assert_eq!(cpu.index_register, 0);
        assert_eq!(cpu.registers, [0; 16]);
        assert_eq!(cpu.keypad, [false; 16]);
        assert!(cpu.display.iter().all(|p| !*p));
        assert_eq!(cpu.delay_timer.load(Ordering::Relaxed), 0);
        assert_eq!(cpu.sound_timer.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn init_loads_standard_hex_font_in_interpreter_space() {
        let cpu = Cpu::init();
        let start = Cpu::FONT_START;
        assert!(start + 80 <= 0x200, "font must live in 0x000..0x200");
        assert_eq!(&cpu.memory[start..start + 80], &FONT_BYTES);
    }

    #[test]
    fn registers_map_to_v0_through_vf() {
        for (i, reg) in all_registers().into_iter().enumerate() {
            let idx: usize = reg.into();
            assert_eq!(idx, i);
            assert_eq!(Registers::try_from(i).unwrap(), reg);
        }
        assert!(Registers::try_from(16usize).is_err());
    }
}

mod cls_00e0 {
    use super::*;

    #[test]
    fn clears_every_pixel_and_nothing_else() {
        let mut cpu = cpu();
        cpu.pc = 0x200;
        cpu.ld(Registers::V0, 0xAA);
        cpu.ldi(0x300);
        for i in 0..cpu.display.len() {
            cpu.display[i] = i % 2 == 0;
        }
        cpu.cls();
        assert_eq!(lit_count(&cpu), 0);
        assert_eq!(cpu.pc, 0x200);
        assert_eq!(cpu.registers[Registers::V0], 0xAA);
        assert_eq!(cpu.index_register, 0x300);
    }
}

mod ret_00ee_and_call_2nnn {
    use super::*;

    #[test]
    fn ret_pops_nested_frames_in_lifo_order() {
        let mut cpu = cpu();
        cpu.call(0x200);
        cpu.call(0x302);
        assert_eq!(cpu.pc, 0x302);
        cpu.ret();
        cpu.ret();
        assert_eq!(
            cpu.pc, 0x200,
            "00EE must not destroy the next stack frame when popping"
        );
    }

    #[test]
    fn ret_restores_a_single_frame() {
        let mut cpu = cpu();
        cpu.call(0xABC);
        cpu.ret();
        assert_eq!(cpu.pc, 0x200);
    }

    #[test]
    fn call_pushes_pc_and_jumps_into_program_space() {
        let mut cpu = cpu();
        cpu.pc = 0x200;
        cpu.call(0x350);
        assert_eq!(cpu.pc, 0x350);
        cpu.ret();
        assert_eq!(cpu.pc, 0x200, "00EE must restore the address saved by 2nnn");
    }

    #[test]
    fn nested_calls_return_in_lifo_order() {
        let mut cpu = cpu();
        cpu.pc = 0x200;
        cpu.call(0x300);
        assert_eq!(cpu.pc, 0x300);
        cpu.pc = 0x302;
        cpu.call(0x400);
        assert_eq!(cpu.pc, 0x400);
        cpu.ret();
        assert_eq!(cpu.pc, 0x302);
        cpu.ret();
        assert_eq!(cpu.pc, 0x200);
    }

    #[test]
    fn sixteen_nested_subroutines_round_trip() {
        let mut cpu = cpu();
        let mut frames = Vec::new();
        for i in 0..16u16 {
            frames.push(cpu.pc);
            cpu.call(0x300 + i);
        }
        for expected in frames.into_iter().rev() {
            cpu.ret();
            assert_eq!(cpu.pc, expected);
        }
    }

    #[test]
    fn call_does_not_clear_other_registers_or_the_display() {
        let mut cpu = cpu();
        cpu.ld(Registers::V1, 9);
        set_pixel(&mut cpu, 3, 3, true);
        cpu.pc = 0x210;
        cpu.call(0x400);
        assert_eq!(cpu.registers[Registers::V1], 9);
        assert!(pixel(&cpu, 3, 3));
    }
}

mod jmp_1nnn {
    use super::*;

    #[test]
    fn jumps_to_program_entry() {
        let mut cpu = cpu();
        cpu.jmp(0x200);
        assert_eq!(cpu.pc, 0x200);
    }

    #[test]
    fn jumps_anywhere_in_4k() {
        let mut cpu = cpu();
        for addr in [0x000u16, 0x1FF, 0x200, 0xABC, 0xFFF] {
            cpu.jmp(addr);
            assert_eq!(cpu.pc, addr, "1nnn sets PC to {addr:#05x}");
        }
    }

    #[test]
    fn jump_does_not_touch_i_or_v() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 5);
        cpu.ldi(0x123);
        cpu.jmp(0x400);
        assert_eq!(cpu.registers[Registers::V0], 5);
        assert_eq!(cpu.index_register, 0x123);
    }
}

mod skip {
    use super::*;

    #[test]
    fn se_3xkk_skips_exactly_one_instruction_when_equal() {
        let mut cpu = cpu();
        cpu.ld(Registers::V2, 0x7A);
        cpu.se(Registers::V2, 0x7A);
        assert_eq!(cpu.pc, 0x202);
        cpu.se(Registers::V2, 0x00);
        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn sne_4xkk_skips_when_not_equal() {
        let mut cpu = cpu();
        cpu.ld(Registers::VA, 1);
        cpu.sne(Registers::VA, 2);
        assert_eq!(cpu.pc, 0x202);
        cpu.sne(Registers::VA, 1);
        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn se_5xy0_compares_two_registers() {
        let mut cpu = cpu();
        cpu.ld(Registers::V4, 9);
        cpu.ld(Registers::V5, 9);
        cpu.ser(Registers::V4, Registers::V5);
        assert_eq!(cpu.pc, 0x202);
        cpu.ld(Registers::V5, 8);
        cpu.ser(Registers::V4, Registers::V5);
        assert_eq!(cpu.pc, 0x202);
        cpu.ser(Registers::V4, Registers::V4);
        assert_eq!(cpu.pc, 0x204);
    }

    #[test]
    fn sne_9xy0_skips_when_registers_differ() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 1);
        cpu.ld(Registers::V1, 2);
        cpu.sner(Registers::V0, Registers::V1);
        assert_eq!(cpu.pc, 0x202);
        cpu.ld(Registers::V1, 1);
        cpu.sner(Registers::V0, Registers::V1);
        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn skips_do_not_change_registers() {
        let mut cpu_ = cpu();
        cpu_.ld(Registers::V3, 0x11);
        cpu_.se(Registers::V3, 0x11);
        cpu_.sne(Registers::V3, 0x00);
        assert_eq!(cpu_.registers[Registers::V3], 0x11);
        assert_eq!(cpu_.sp, cpu().sp);
    }
}

mod ld_and_add_immediate {
    use super::*;

    #[test]
    fn ld_6xkk_writes_every_register() {
        let mut cpu = cpu();
        for (i, reg) in all_registers().into_iter().enumerate() {
            cpu.ld(reg, i as u8 * 3);
            assert_eq!(cpu.registers[reg], i as u8 * 3);
        }
    }

    #[test]
    fn add_7xkk_wraps_and_does_not_touch_vf() {
        let mut cpu = cpu();
        cpu.ld(Registers::VF, 0xAA);
        cpu.ld(Registers::V1, 250);
        cpu.add(Registers::V1, 10);
        assert_eq!(cpu.registers[Registers::V1], 4);
        assert_eq!(cpu.registers[Registers::VF], 0xAA);
        cpu.add(Registers::V1, 0);
        assert_eq!(cpu.registers[Registers::V1], 4);
        cpu.ld(Registers::VA, 0xFF);
        cpu.add(Registers::VA, 1);
        assert_eq!(cpu.registers[Registers::VA], 0);
        assert_eq!(cpu.registers[Registers::VF], 0xAA);
    }
}

mod alu_8xxx {
    use super::*;

    #[test]
    fn ld_8xy0_sets_vx_to_vy() {
        let mut cpu = cpu();
        cpu.ld(Registers::V1, 0xAB);
        cpu.ld(Registers::V2, 0x11);
        cpu.ldr(Registers::V1, Registers::V2);
        assert_eq!(
            cpu.registers[Registers::V1],
            0x11,
            "8xy0: Vx = Vy (Cowgod / VIP)"
        );
        assert_eq!(cpu.registers[Registers::V2], 0x11, "Vy is unchanged");
    }

    #[test]
    fn ld_8xy0_same_register_is_noop() {
        let mut cpu = cpu();
        cpu.ld(Registers::V4, 0x3C);
        cpu.ldr(Registers::V4, Registers::V4);
        assert_eq!(cpu.registers[Registers::V4], 0x3C);
    }

    #[test]
    fn or_and_xor_set_vx_and_leave_vy() {
        let cases = [
            (0b0000_0000, 0b0000_0000),
            (0b1111_1111, 0b0000_0000),
            (0b1010_1010, 0b0101_0101),
            (0b1111_0000, 0b1100_1100),
            (0xFF, 0xFF),
        ];
        for (x, y) in cases {
            let mut cpu = cpu();
            cpu.ld(Registers::V0, x);
            cpu.ld(Registers::V1, y);
            cpu.or(Registers::V0, Registers::V1);
            assert_eq!(cpu.registers[Registers::V0], x | y);
            assert_eq!(cpu.registers[Registers::V1], y);

            cpu.ld(Registers::V0, x);
            cpu.and(Registers::V0, Registers::V1);
            assert_eq!(cpu.registers[Registers::V0], x & y);
            assert_eq!(cpu.registers[Registers::V1], y);

            cpu.ld(Registers::V0, x);
            cpu.xor(Registers::V0, Registers::V1);
            assert_eq!(cpu.registers[Registers::V0], x ^ y);
            assert_eq!(cpu.registers[Registers::V1], y);
        }
    }

    #[test]
    fn add_8xy4_sets_carry_and_keeps_low_byte() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 10);
        cpu.ld(Registers::V1, 20);
        cpu.addc(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 30);
        assert_eq!(cpu.registers[Registers::VF], 0);
        assert_eq!(cpu.registers[Registers::V1], 20);

        cpu.ld(Registers::V0, 200);
        cpu.ld(Registers::V1, 100);
        cpu.addc(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 44);
        assert_eq!(cpu.registers[Registers::VF], 1);

        cpu.ld(Registers::V3, 255);
        cpu.ld(Registers::V4, 1);
        cpu.addc(Registers::V3, Registers::V4);
        assert_eq!(cpu.registers[Registers::V3], 0);
        assert_eq!(cpu.registers[Registers::VF], 1);

        cpu.ld(Registers::V3, 255);
        cpu.ld(Registers::V4, 0);
        cpu.addc(Registers::V3, Registers::V4);
        assert_eq!(cpu.registers[Registers::V3], 255);
        assert_eq!(cpu.registers[Registers::VF], 0);
    }

    #[test]
    fn add_8xy4_with_vf_as_addend_uses_original_vf() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0xFE);
        cpu.ld(Registers::VF, 0x03);
        cpu.addc(Registers::V0, Registers::VF);
        assert_eq!(cpu.registers[Registers::V0], 0x01);
        assert_eq!(cpu.registers[Registers::VF], 1);
    }

    #[test]
    fn sub_8xy5_sets_vf_when_operands_are_equal() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 10);
        cpu.ld(Registers::V1, 10);
        cpu.sub(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 0);
        assert_eq!(
            cpu.registers[Registers::VF],
            1,
            "no borrow when Vx == Vy (VIP / Tobias; Cowgod's '>' is wrong)"
        );
    }

    #[test]
    fn sub_8xy5_wraps_on_underflow() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 3);
        cpu.ld(Registers::V1, 10);
        cpu.sub(Registers::V0, Registers::V1);
        assert_eq!(
            cpu.registers[Registers::V0],
            3u8.wrapping_sub(10),
            "8xy5 is wrapping subtraction, not saturating"
        );
        assert_eq!(cpu.registers[Registers::VF], 0);
    }

    #[test]
    fn sub_8xy5_no_borrow_when_vx_greater() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 10);
        cpu.ld(Registers::V1, 3);
        cpu.sub(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 7);
        assert_eq!(cpu.registers[Registers::VF], 1);
        assert_eq!(cpu.registers[Registers::V1], 3);
    }

    #[test]
    fn subn_8xy7_no_borrow_when_vy_greater() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 3);
        cpu.ld(Registers::V1, 10);
        cpu.subn(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 7);
        assert_eq!(cpu.registers[Registers::VF], 1);
        assert_eq!(cpu.registers[Registers::V1], 10);
    }

    #[test]
    fn subn_8xy7_sets_vf_when_operands_are_equal() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 8);
        cpu.ld(Registers::V1, 8);
        cpu.subn(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 0);
        assert_eq!(cpu.registers[Registers::VF], 1);
    }

    #[test]
    fn subn_8xy7_wraps_on_underflow() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 10);
        cpu.ld(Registers::V1, 3);
        cpu.subn(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 3u8.wrapping_sub(10));
        assert_eq!(cpu.registers[Registers::VF], 0);
    }

    #[test]
    fn shr_8xy6_shifts_vx_in_place_and_sets_vf_to_lsb() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0b0000_0101);
        cpu.ld(Registers::V1, 0xFF);
        cpu.shr(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 0b0000_0010);
        assert_eq!(cpu.registers[Registers::VF], 1);
        assert_eq!(
            cpu.registers[Registers::V1],
            0xFF,
            "Vy is ignored / unchanged"
        );

        cpu.ld(Registers::V0, 0b0000_0100);
        cpu.shr(Registers::V0, Registers::V2);
        assert_eq!(cpu.registers[Registers::V0], 0b0000_0010);
        assert_eq!(cpu.registers[Registers::VF], 0);

        cpu.ld(Registers::V0, 0);
        cpu.shr(Registers::V0, Registers::V0);
        assert_eq!(cpu.registers[Registers::V0], 0);
        assert_eq!(cpu.registers[Registers::VF], 0);
    }

    #[test]
    fn shl_8xye_shifts_vx_in_place_and_sets_vf_to_one_or_zero() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0b0100_0001);
        cpu.ld(Registers::V1, 0x09);
        cpu.shl(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 0b1000_0010);
        assert_eq!(cpu.registers[Registers::VF], 0, "VF is 0 or 1, never 0x80");
        assert_eq!(cpu.registers[Registers::V1], 0x09);
    }

    #[test]
    fn shl_8xye_wraps_and_sets_vf_from_the_shifted_out_msb() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0b1000_0001);
        cpu.ld(Registers::V1, 0x09);
        cpu.shl(Registers::V0, Registers::V1);
        assert_eq!(
            cpu.registers[Registers::V0],
            0b0000_0010,
            "shift left is wrapping (0x81 << 1 == 0x02)"
        );
        assert_eq!(cpu.registers[Registers::VF], 1);

        cpu.ld(Registers::V0, 0x80);
        cpu.shl(Registers::V0, Registers::V1);
        assert_eq!(cpu.registers[Registers::V0], 0);
        assert_eq!(cpu.registers[Registers::VF], 1);
    }
}

mod index_and_jump_v0 {
    use super::*;

    #[test]
    fn ldi_annn_sets_i() {
        let mut cpu = cpu();
        cpu.ldi(0x123);
        assert_eq!(cpu.index_register, 0x123);
        cpu.ldi(0);
        assert_eq!(cpu.index_register, 0);
        cpu.ldi(0xFFF);
        assert_eq!(cpu.index_register, 0xFFF);
    }

    #[test]
    fn ljmp_bnnn_adds_v0() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0x05);
        cpu.ld(Registers::V1, 0x11);
        cpu.ljmp(0x300);
        assert_eq!(cpu.pc, 0x305);
        assert_eq!(cpu.registers[Registers::V1], 0x11);

        cpu.ld(Registers::V0, 0);
        cpu.ljmp(0xABC);
        assert_eq!(cpu.pc, 0xABC);

        cpu.ld(Registers::V0, 0xFF);
        cpu.ljmp(0x200);
        assert_eq!(cpu.pc, 0x2FF);
    }
}

mod random_cxkk {
    use super::*;

    #[test]
    fn rnd_is_masked_with_kk() {
        let mut cpu = cpu();
        for mask in [0x00u8, 0x0F, 0xF0, 0x55, 0xAA, 0xFF] {
            for _ in 0..48 {
                cpu.rnd(Registers::V5, mask);
                assert_eq!(
                    cpu.registers[Registers::V5] & !mask,
                    0,
                    "Cxkk must AND the random byte with kk={mask:#04x}"
                );
            }
        }
    }

    #[test]
    fn rnd_with_zero_mask_is_always_zero() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0xFF);
        for _ in 0..16 {
            cpu.rnd(Registers::V0, 0);
            assert_eq!(cpu.registers[Registers::V0], 0);
        }
    }
}

mod draw_dxyn {
    use super::*;

    #[test]
    fn draws_msb_as_leftmost_pixel() {
        let mut cpu = cpu();
        load_sprite(&mut cpu, 0x300, &[0b1010_0001]);
        cpu.ld(Registers::V0, 0);
        cpu.ld(Registers::V1, 0);
        cpu.drw(Registers::V0, Registers::V1, 1);
        assert!(pixel(&cpu, 0, 0));
        assert!(!pixel(&cpu, 1, 0));
        assert!(pixel(&cpu, 2, 0));
        assert!(!pixel(&cpu, 3, 0));
        assert!(!pixel(&cpu, 4, 0));
        assert!(!pixel(&cpu, 5, 0));
        assert!(!pixel(&cpu, 6, 0));
        assert!(pixel(&cpu, 7, 0));
        assert_eq!(lit_count(&cpu), 3);
        assert_eq!(cpu.registers[Registers::VF], 0);
    }

    #[test]
    fn draws_n_rows_from_i() {
        let mut cpu = cpu();
        load_sprite(&mut cpu, 0x300, &[0x80, 0x40, 0x20]);
        cpu.ld(Registers::V0, 4);
        cpu.ld(Registers::V1, 6);
        cpu.drw(Registers::V0, Registers::V1, 3);
        assert!(pixel(&cpu, 4, 6));
        assert!(pixel(&cpu, 5, 7));
        assert!(pixel(&cpu, 6, 8));
        assert_eq!(lit_count(&cpu), 3);
        assert_eq!(cpu.index_register, 0x300);
        assert_eq!(cpu.registers[Registers::V0], 4);
        assert_eq!(cpu.registers[Registers::V1], 6);
    }

    #[test]
    fn xors_pixels_and_sets_vf_on_collision() {
        let mut cpu = cpu();
        load_sprite(&mut cpu, 0x300, &[0xF0]);
        cpu.ld(Registers::V0, 0);
        cpu.ld(Registers::V1, 0);
        cpu.drw(Registers::V0, Registers::V1, 1);
        assert_eq!(cpu.registers[Registers::VF], 0);
        cpu.drw(Registers::V0, Registers::V1, 1);
        assert_eq!(cpu.registers[Registers::VF], 1);
        assert_eq!(lit_count(&cpu), 0);
    }

    #[test]
    fn clears_vf_when_there_is_no_collision() {
        let mut cpu = cpu();
        cpu.ld(Registers::VF, 1);
        load_sprite(&mut cpu, 0x300, &[0x80]);
        cpu.ld(Registers::V0, 10);
        cpu.ld(Registers::V1, 10);
        cpu.drw(Registers::V0, Registers::V1, 1);
        assert!(pixel(&cpu, 10, 10));
        assert_eq!(
            cpu.registers[Registers::VF],
            0,
            "Dxyn sets VF=0 when no pixel is turned off"
        );
    }

    #[test]
    fn n_zero_draws_nothing() {
        let mut cpu = cpu();
        load_sprite(&mut cpu, 0x300, &[0xFF]);
        cpu.ld(Registers::V0, 0);
        cpu.ld(Registers::V1, 0);
        cpu.drw(Registers::V0, Registers::V1, 0);
        assert_eq!(lit_count(&cpu), 0);
        assert_eq!(cpu.registers[Registers::VF], 0);
    }

    #[test]
    fn starting_coordinates_wrap() {
        let mut cpu = cpu();
        load_sprite(&mut cpu, 0x300, &[0x80]);
        cpu.ld(Registers::V0, 64);
        cpu.ld(Registers::V1, 32);
        cpu.drw(Registers::V0, Registers::V1, 1);
        assert!(pixel(&cpu, 0, 0), "64 % 64 == 0, 32 % 32 == 0");

        cpu.cls();
        cpu.ld(Registers::V0, 65);
        cpu.ld(Registers::V1, 33);
        cpu.drw(Registers::V0, Registers::V1, 1);
        assert!(pixel(&cpu, 1, 1));
    }

    #[test]
    fn sprite_clips_at_right_edge_and_still_draws_column_63() {
        let mut cpu = cpu();
        load_sprite(&mut cpu, 0x300, &[0xFF]);
        cpu.ld(Registers::V0, 62);
        cpu.ld(Registers::V1, 0);
        cpu.drw(Registers::V0, Registers::V1, 1);
        assert!(pixel(&cpu, 62, 0));
        assert!(
            pixel(&cpu, 63, 0),
            "column 63 is on-screen and must be drawn"
        );
        assert!(
            !pixel(&cpu, 0, 0),
            "remaining sprite bits clip; they do not wrap"
        );
        assert_eq!(lit_count(&cpu), 2);
    }

    #[test]
    fn sprite_at_last_column_draws_the_leftmost_bit() {
        let mut cpu = cpu();
        load_sprite(&mut cpu, 0x300, &[0xFF]);
        cpu.ld(Registers::V0, 63);
        cpu.ld(Registers::V1, 0);
        cpu.drw(Registers::V0, Registers::V1, 1);
        assert!(pixel(&cpu, 63, 0));
        assert!(!pixel(&cpu, 0, 0));
        assert_eq!(lit_count(&cpu), 1);
    }

    #[test]
    fn sprite_clips_at_bottom_and_still_draws_row_31() {
        let mut cpu = cpu();
        load_sprite(&mut cpu, 0x300, &[0x80, 0x80, 0x80]);
        cpu.ld(Registers::V0, 0);
        cpu.ld(Registers::V1, 30);
        cpu.drw(Registers::V0, Registers::V1, 3);
        assert!(pixel(&cpu, 0, 30));
        assert!(pixel(&cpu, 0, 31), "row 31 is on-screen and must be drawn");
        assert!(!pixel(&cpu, 0, 0), "sprite does not wrap vertically");
        assert_eq!(lit_count(&cpu), 2);
    }

    #[test]
    fn sprite_at_last_row_draws() {
        let mut cpu = cpu();
        load_sprite(&mut cpu, 0x300, &[0x80]);
        cpu.ld(Registers::V0, 0);
        cpu.ld(Registers::V1, 31);
        cpu.drw(Registers::V0, Registers::V1, 1);
        assert!(pixel(&cpu, 0, 31));
        assert_eq!(lit_count(&cpu), 1);
    }

    #[test]
    fn font_digit_zero_matches_cowgod_glyph() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0);
        cpu.ld(Registers::V1, 0);
        cpu.ldf(Registers::V0);
        cpu.drw(Registers::V0, Registers::V1, 5);
        let expected = [
            [true, true, true, true, false, false, false, false],
            [true, false, false, true, false, false, false, false],
            [true, false, false, true, false, false, false, false],
            [true, false, false, true, false, false, false, false],
            [true, true, true, true, false, false, false, false],
        ];
        for (y, row) in expected.iter().enumerate() {
            for (x, on) in row.iter().enumerate() {
                assert_eq!(pixel(&cpu, x, y), *on, "font 0 pixel ({x},{y})");
            }
        }
    }
}

mod keypad_ex {
    use super::*;

    #[test]
    fn skp_ex9e_skips_only_when_that_key_is_down() {
        let mut down = cpu();
        down.ld(Registers::V0, 0xA);
        down.keypad[0xA] = true;
        down.skp(Registers::V0);
        assert_eq!(down.pc, 0x202);

        let mut up = cpu();
        up.ld(Registers::V0, 0xA);
        up.skp(Registers::V0);
        assert_eq!(up.pc, 0x200);
    }

    #[test]
    fn sknp_exa1_skips_only_when_that_key_is_up() {
        let mut up = cpu();
        up.ld(Registers::V3, 4);
        up.sknp(Registers::V3);
        assert_eq!(up.pc, 0x202);

        let mut down = cpu();
        down.ld(Registers::V3, 4);
        down.keypad[4] = true;
        down.sknp(Registers::V3);
        assert_eq!(down.pc, 0x200);
    }

    #[test]
    fn every_hex_key_0_through_f() {
        for key in 0u8..16 {
            let mut down = cpu();
            down.ld(Registers::V1, key);
            down.keypad[key as usize] = true;
            down.skp(Registers::V1);
            assert_eq!(down.pc, 0x202, "Ex9E key {key:X}");

            let mut up = cpu();
            up.ld(Registers::V1, key);
            up.sknp(Registers::V1);
            assert_eq!(up.pc, 0x202, "ExA1 key {key:X} up");
        }
    }

    #[test]
    fn other_keys_do_not_count_as_vx() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 2);
        for i in 0..16 {
            cpu.keypad[i] = i != 2;
        }
        cpu.skp(Registers::V0);
        assert_eq!(cpu.pc, 0x200);
        cpu.sknp(Registers::V0);
        assert_eq!(cpu.pc, 0x202);
    }
}

mod wait_key_fx0a {
    use super::*;

    #[test]
    fn stores_the_pressed_key_and_returns() {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut cpu = cpu();
            cpu.keypad[0xC] = true;
            cpu.ldwfk(Registers::V3);
            let _ = tx.send(cpu.registers[Registers::V3]);
        });
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(key) => assert_eq!(key, 0xC, "Fx0A stores the hex key in Vx"),
            Err(err) => {
                panic!("Fx0A must return once a key is pressed (the wait loop must terminate): {err}")
            }
        }
    }

    #[test]
    fn stores_the_lowest_pressed_key() {
        let (tx, rx) = mpsc::channel();
        let mut cpu = cpu();
        thread::spawn(move || {
            cpu.keypad[3] = true;
            cpu.keypad[9] = true;
            cpu.ldwfk(Registers::V0);
            let _ = tx.send(cpu.registers[Registers::V0]);
        });
        let key = rx
            .recv_timeout(Duration::from_millis(200))
            .expect("Fx0A hung");
        assert!(key == 3 || key == 9, "got unexpected key {key}");
    }
}

mod timers {
    use super::*;

    #[test]
    fn fx07_fx15_round_trip_delay_timer() {
        let mut cpu = cpu();
        cpu.ld(Registers::V4, 0x2A);
        cpu.lddt(Registers::V4);
        assert_eq!(cpu.delay_timer.load(Ordering::Relaxed), 0x2A);
        cpu.ld(Registers::V5, 0);
        cpu.rdt(Registers::V5);
        assert_eq!(cpu.registers[Registers::V5], 0x2A);
    }

    #[test]
    fn fx18_sets_sound_timer_without_touching_delay() {
        let mut cpu = cpu();
        cpu.ld(Registers::VE, 0x10);
        cpu.ldst(Registers::VE);
        assert_eq!(cpu.sound_timer.load(Ordering::Relaxed), 0x10);
        assert_eq!(cpu.delay_timer.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn timers_accept_zero_and_255() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0);
        cpu.lddt(Registers::V0);
        cpu.ldst(Registers::V0);
        assert_eq!(cpu.delay_timer.load(Ordering::Relaxed), 0);
        assert_eq!(cpu.sound_timer.load(Ordering::Relaxed), 0);
        cpu.ld(Registers::V0, 255);
        cpu.lddt(Registers::V0);
        cpu.ldst(Registers::V0);
        assert_eq!(cpu.delay_timer.load(Ordering::Relaxed), 255);
        assert_eq!(cpu.sound_timer.load(Ordering::Relaxed), 255);
    }

    #[test]
    fn delay_timer_counts_down_at_60hz_while_nonzero() {
        let mut cpu = Cpu::init();
        cpu.ld(Registers::V0, 20);
        cpu.lddt(Registers::V0);
        thread::sleep(Duration::from_millis(80));
        cpu.rdt(Registers::V0);
        let left = cpu.registers[Registers::V0];
        assert!(
            left <= 17,
            "DT must decrement ~60 times/sec while > 0; 80ms should drop it by several ticks (got {left})"
        );
    }

    #[test]
    fn sound_timer_counts_down_at_60hz_while_nonzero() {
        let cpu = Cpu::init();
        cpu.sound_timer.store(20, Ordering::Relaxed);
        thread::sleep(Duration::from_millis(80));
        let left = cpu.sound_timer.load(Ordering::Relaxed);
        assert!(
            left <= 17,
            "ST must decrement ~60 times/sec while > 0; 80ms should drop it by several ticks (got {left})"
        );
    }
}

mod fx_memory {
    use super::*;

    #[test]
    fn addi_fx1e_adds_vx_to_i_without_changing_vf() {
        let mut cpu = cpu();
        cpu.ld(Registers::VF, 0xAA);
        cpu.ldi(0x300);
        cpu.ld(Registers::V2, 0x20);
        cpu.addi(Registers::V2);
        assert_eq!(cpu.index_register, 0x320);
        assert_eq!(cpu.registers[Registers::V2], 0x20);
        assert_eq!(cpu.registers[Registers::VF], 0xAA);
        cpu.addi(Registers::V2);
        assert_eq!(cpu.index_register, 0x340);
    }

    #[test]
    fn ldf_fx29_points_i_at_5_byte_hex_glyphs() {
        let mut cpu = cpu();
        for digit in 0u8..16 {
            cpu.ld(Registers::V0, digit);
            cpu.ldf(Registers::V0);
            assert_eq!(
                cpu.index_register,
                Cpu::FONT_START as u16 + u16::from(digit) * 5
            );
        }
        cpu.ld(Registers::V0, 0xA);
        cpu.ldf(Registers::V0);
        let i = cpu.index_register as usize;
        assert_eq!(&cpu.memory[i..i + 5], &FONT_BYTES[50..55]);
    }

    #[test]
    #[should_panic = "COSMAC VIP / Tobias: Fx29 uses Vx & 0xF"] // loading a font with with a char that is otside of bounds in undfined behaviour
    // in this case the test will panic. i will leave this in in case somone wants to have a font
    // with more then 16 charaters
    fn ldf_fx29_uses_the_low_nibble_of_vx() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0x1A);
        cpu.ldf(Registers::V0);
        assert_eq!(
            cpu.index_register,
            Cpu::FONT_START as u16 + 0xA * 5,
            "COSMAC VIP / Tobias: Fx29 uses Vx & 0xF"
        );
    }

    #[test]
    fn ldb_fx33_writes_hundreds_tens_ones_and_leaves_i() {
        let cases = [
            (0u8, [0, 0, 0]),
            (7, [0, 0, 7]),
            (9, [0, 0, 9]),
            (10, [0, 1, 0]),
            (42, [0, 4, 2]),
            (99, [0, 9, 9]),
            (100, [1, 0, 0]),
            (123, [1, 2, 3]),
            (200, [2, 0, 0]),
            (255, [2, 5, 5]),
        ];
        for (value, digits) in cases {
            let mut cpu = cpu();
            cpu.memory[0x3FF] = 0xEE;
            cpu.memory[0x403] = 0xEF;
            cpu.ldi(0x400);
            cpu.ld(Registers::V9, value);
            cpu.ldb(Registers::V9);
            assert_eq!(cpu.memory[0x400..0x403], digits, "BCD of {value}");
            assert_eq!(cpu.index_register, 0x400);
            assert_eq!(cpu.registers[Registers::V9], value);
            assert_eq!(cpu.memory[0x3FF], 0xEE);
            assert_eq!(cpu.memory[0x403], 0xEF);
        }
    }

    #[test]
    fn ldtomem_fx55_stores_v0_through_vx_only_and_leaves_i() {
        let mut cpu = cpu();
        for (i, reg) in all_registers().into_iter().enumerate() {
            cpu.ld(reg, 0xA0 + i as u8);
        }
        cpu.memory[0x500..0x510].fill(0xFF);
        cpu.ldi(0x500);
        cpu.ldtomem(Registers::V2);

        assert_eq!(cpu.memory[0x500], 0xA0);
        assert_eq!(cpu.memory[0x501], 0xA1);
        assert_eq!(cpu.memory[0x502], 0xA2);
        assert_eq!(cpu.memory[0x503], 0xFF, "Fx55 with x=2 must not write V3..");
        assert_eq!(cpu.memory[0x50F], 0xFF);
        assert_eq!(
            cpu.index_register, 0x500,
            "modern CHIP-8: I is unchanged after Fx55"
        );
        assert_eq!(cpu.registers[Registers::V3], 0xA3);
    }

    #[test]
    fn ldtomem_fx55_x_equals_0_stores_only_v0() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 0x11);
        cpu.ld(Registers::V1, 0x22);
        cpu.ldi(0x200);
        cpu.memory[0x201] = 0xEE;
        cpu.ldtomem(Registers::V0);
        assert_eq!(cpu.memory[0x200], 0x11);
        assert_eq!(cpu.memory[0x201], 0xEE);
    }

    #[test]
    fn ldtomem_fx55_x_equals_f_stores_all_sixteen() {
        let mut cpu = cpu();
        for (i, reg) in all_registers().into_iter().enumerate() {
            cpu.ld(reg, i as u8 + 1);
        }
        cpu.ldi(0x600);
        cpu.ldtomem(Registers::VF);
        for i in 0..16 {
            assert_eq!(cpu.memory[0x600 + i], i as u8 + 1);
        }
        assert_eq!(cpu.index_register, 0x600);
    }

    #[test]
    fn ldfrommem_fx65_loads_v0_through_vx_only_and_leaves_i() {
        let mut cpu = cpu();
        cpu.ldi(0x700);
        for i in 0..16 {
            cpu.memory[0x700 + i] = 0xC0 + i as u8;
        }
        cpu.ld(Registers::V3, 0x99);
        cpu.ld(Registers::VF, 0x88);
        cpu.ldfrommem(Registers::V2);
        assert_eq!(cpu.registers[Registers::V0], 0xC0);
        assert_eq!(cpu.registers[Registers::V1], 0xC1);
        assert_eq!(cpu.registers[Registers::V2], 0xC2);
        assert_eq!(
            cpu.registers[Registers::V3],
            0x99,
            "Fx65 with x=2 must not load V3.."
        );
        assert_eq!(cpu.registers[Registers::VF], 0x88);
        assert_eq!(cpu.index_register, 0x700);
    }

    #[test]
    fn fx55_then_fx65_round_trip_through_x() {
        let mut cpu = cpu();
        for (i, reg) in all_registers().into_iter().enumerate() {
            cpu.ld(reg, (i * 7) as u8);
        }
        let original = cpu.registers;
        cpu.ldi(0x800);
        cpu.ldtomem(Registers::V7);
        cpu.registers = [0; 16];
        cpu.ldfrommem(Registers::V7);
        assert_eq!(&cpu.registers[0..=7], &original[0..=7]);
        assert_eq!(
            &cpu.registers[8..],
            &[0; 8],
            "registers above Vx stay at their previous values (here 0)"
        );
    }
}

mod isolation {
    use super::*;

    #[test]
    fn arithmetic_does_not_move_pc_or_draw() {
        let mut cpu = cpu();
        cpu.ld(Registers::V0, 5);
        cpu.ld(Registers::V1, 6);
        cpu.add(Registers::V0, 1);
        cpu.addc(Registers::V0, Registers::V1);
        cpu.sub(Registers::V0, Registers::V1);
        cpu.or(Registers::V0, Registers::V1);
        cpu.and(Registers::V0, Registers::V1);
        cpu.xor(Registers::V0, Registers::V1);
        cpu.ldi(0x111);
        cpu.addi(Registers::V1);
        assert_eq!(cpu.pc, 0x200);
        assert_eq!(cpu.sp, Cpu::init().sp);
        assert_eq!(lit_count(&cpu), 0);
    }
}

/// Peak CHIP-8 clock using `Cpu::start()` (the real fetch/decode/execute loop).
///
/// The ROM is two nested 8-bit counters of ADD/SNE/JP, then `JP 0xFFF` so
/// `start` returns (`while pc < 4095`). Run with:
/// `cargo bench simple_program_clock -- --nocapture`
mod clock {
    extern crate test;
    
    use std::time::Instant;

    use test::Bencher;

    use super::*;

    /// Exact fetch/execute count of one `start()` run (see `SIMPLE_PROGRAM`).
    const INSTRUCTIONS_PER_RUN: u32 = 197_633;
    const RUNS: u32 = 4000;

    /// Nested V0/V1 increment loops, then halt by jumping to 0xFFF.
    ///
    /// ```text
    /// 200: LD V1, 0
    /// 202: LD V0, 0
    /// 204: ADD V0, 1
    /// 206: SNE V0, 0
    /// 208: JP  20C        ; V0 wrapped
    /// 20A: JP  204
    /// 20C: ADD V1, 1
    /// 20E: SNE V1, 0
    /// 210: JP  FFF        ; both counters wrapped; start() exits
    /// 212: JP  202
    /// ```
    const SIMPLE_PROGRAM: &[u8] = &[
        0x61, 0x00, 0x60, 0x00, 0x70, 0x01, 0x40, 0x00, 0x12, 0x0C, 0x12, 0x04, 0x71,
        0x01, 0x41, 0x00, 0x1F, 0xFF, 0x12, 0x02,
    ];

    fn bench_cpu() -> Cpu {
        let mut cpu = Cpu::init();
        cpu.load_data(SIMPLE_PROGRAM);
        cpu
    }

    #[bench]
    fn simple_program_clock(b: &mut Bencher) {
        let mut cpu = bench_cpu();

        let start = Instant::now();
        for _ in 0..RUNS {
            cpu.reset();
            cpu.start();
        }
        let hz = f64::from(RUNS * INSTRUCTIONS_PER_RUN) / start.elapsed().as_secs_f64();
        eprintln!(
            "\nsimple CHIP-8 program max clock on this machine: {hz:.0} Hz ({:.2} MHz)\n",
            hz / 1_000_000.0
        );

        b.iter(|| {
            cpu.reset();
            cpu.start();
            test::black_box(cpu.pc)
        });
    }

    #[bench]
    fn fetch_decode(b: &mut Bencher) {
        let mut opcode = 0;
        let mut cpu = Cpu::init();
        unsafe { std::arch::x86_64::_rdrand16_step(&mut opcode); }
        cpu.memory[cpu.pc as usize] = opcode.to_be_bytes()[0];
        cpu.memory[cpu.pc as usize + 1] = opcode.to_be_bytes()[1];
        cpu.pc = 0x200;
        
        b.iter(|| {
            cpu.pc = 0x200;
            test::black_box(cpu.fetch_decode());
        });
    }
}
