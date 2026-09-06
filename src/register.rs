#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Registers {
    V0,
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
    V8,
    V9,
    VA,
    VB,
    VC,
    VD,
    VE,
    VF,
}

impl std::ops::Index<Registers> for [u8] {
    type Output = u8;

    fn index(&self, index: Registers) -> &Self::Output {
        &self[<Registers as std::convert::Into<usize>>::into(index)]
    }
}

impl std::ops::IndexMut<Registers> for [u8] {
    fn index_mut(&mut self, index: Registers) -> &mut Self::Output {
        &mut self[<Registers as std::convert::Into<usize>>::into(index)]
    }
}

#[allow(clippy::from_over_into)]
impl Into<usize> for Registers {
    fn into(self) -> usize {
        match self {
            Registers::V0 => 0x0,
            Registers::V1 => 0x1,
            Registers::V2 => 0x2,
            Registers::V3 => 0x3,
            Registers::V4 => 0x4,
            Registers::V5 => 0x5,
            Registers::V6 => 0x6,
            Registers::V7 => 0x7,
            Registers::V8 => 0x8,
            Registers::V9 => 0x9,
            Registers::VA => 0xa,
            Registers::VB => 0xb,
            Registers::VC => 0xc,
            Registers::VD => 0xd,
            Registers::VE => 0xe,
            Registers::VF => 0xf,
        }
    }
}

impl TryFrom<usize> for Registers {
    type Error = std::io::Error;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0x0 => Ok(Registers::V0),
            0x1 => Ok(Registers::V1),
            0x2 => Ok(Registers::V2),
            0x3 => Ok(Registers::V3),
            0x4 => Ok(Registers::V4),
            0x5 => Ok(Registers::V5),
            0x6 => Ok(Registers::V6),
            0x7 => Ok(Registers::V7),
            0x8 => Ok(Registers::V8),
            0x9 => Ok(Registers::V9),
            0xa => Ok(Registers::VA),
            0xb => Ok(Registers::VB),
            0xc => Ok(Registers::VC),
            0xd => Ok(Registers::VD),
            0xe => Ok(Registers::VE),
            0xf => Ok(Registers::VF),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid register",
            )),
        }
    }
}
