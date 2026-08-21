//! SocId parsing and stage description for Amlogic USB boot.

use rootcause::{Result, bail};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocId {
    pub rom_major: u8,
    pub rom_minor: u8,
    pub stage_major: u8,
    pub stage_minor: u8,
    pub need_password: bool,
    pub password_ok: bool,
}

impl SocId {
    pub const STAGE_MINOR_ROM: u8 = 0;
    pub const STAGE_MINOR_BL2: u8 = 1;
    pub const STAGE_MINOR_TPL: u8 = 2;
    pub const STAGE_MINOR_USB_BURNING: u8 = 0x10;
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 6 {
            bail!(
                "SocId response too short: got {} bytes, expected >= 6",
                bytes.len()
            );
        }
        Ok(Self {
            rom_major: bytes[0],
            rom_minor: bytes[1],
            stage_major: bytes[2],
            stage_minor: bytes[3],
            need_password: bytes[4] != 0,
            password_ok: bytes[5] != 0,
        })
    }

    pub fn stage_name(&self) -> &'static str {
        match self.stage_minor {
            Self::STAGE_MINOR_ROM => "MaskROM (Stage 0)",
            Self::STAGE_MINOR_BL2 => "BL2 (Stage 1)",
            Self::STAGE_MINOR_TPL => "TPL / U-Boot Burn Mode (Stage 2)",
            Self::STAGE_MINOR_USB_BURNING => "USB Boot / Burning Mode (0x10)",
            _ => "Unknown Stage",
        }
    }
}

impl fmt::Display for SocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ROM {}.{}, Stage {}.{} ({}), Need Password: {}, Password OK: {}",
            self.rom_major,
            self.rom_minor,
            self.stage_major,
            self.stage_minor,
            self.stage_name(),
            if self.need_password { 1 } else { 0 },
            if self.password_ok { 1 } else { 0 },
        )
    }
}
