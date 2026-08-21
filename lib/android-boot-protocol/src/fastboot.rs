//! Fastboot client protocol implementation (`no_std`).
//!
//! Implements the Android Fastboot protocol over any `Read + Write` stream:
//! - Command execution (`getvar`, `boot`, `flash`, `erase`, `reboot`, etc.)
//! - Data downloading (`download`, streaming download)
//! - Parsing responses (`OKAY`, `FAIL`, `INFO`, `DATA`, `TEXT`)

use crate::error::{Error, Result};
use alloc::format;
use alloc::io::{Read, Write};
use alloc::string::{String, ToString};
use core::str;

/// Response received from a Fastboot device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastbootResponse {
    /// Command succeeded with optional response text.
    Okay(String),
    /// Command failed with error message.
    Fail(String),
    /// Informational message from device (client should continue reading responses).
    Info(String),
    /// Device expects `u32` bytes of data to follow.
    Data(u32),
    /// Text message from device (similar to INFO).
    Text(String),
}

/// Fastboot client for interacting with Android bootloaders over a transport stream.
#[derive(Debug, Default, Clone, Copy)]
pub struct FastbootClient;

impl FastbootClient {
    /// Creates a new `FastbootClient`.
    pub const fn new() -> Self {
        Self
    }

    /// Reads a single response packet from the transport.
    pub fn read_response<T: Read>(&self, transport: &mut T) -> Result<FastbootResponse> {
        let mut buf = [0u8; 1024];
        let n = transport.read(&mut buf)?;
        if n < 4 {
            return Err(Error::FastbootProtocolError(
                "response too short (less than 4 bytes)",
            ));
        }

        let prefix = &buf[..4];
        let payload = &buf[4..n];

        match prefix {
            b"OKAY" => {
                let msg = str::from_utf8(payload)
                    .map_err(|_| Error::Utf8Error)?
                    .to_string();
                Ok(FastbootResponse::Okay(msg))
            }
            b"FAIL" => {
                let msg = str::from_utf8(payload)
                    .map_err(|_| Error::Utf8Error)?
                    .to_string();
                Ok(FastbootResponse::Fail(msg))
            }
            b"INFO" => {
                let msg = str::from_utf8(payload)
                    .map_err(|_| Error::Utf8Error)?
                    .to_string();
                Ok(FastbootResponse::Info(msg))
            }
            b"TEXT" => {
                let msg = str::from_utf8(payload)
                    .map_err(|_| Error::Utf8Error)?
                    .to_string();
                Ok(FastbootResponse::Text(msg))
            }
            b"DATA" => {
                if payload.len() < 8 {
                    return Err(Error::FastbootProtocolError(
                        "DATA response missing 8-hex-digit size",
                    ));
                }
                let hex_str = str::from_utf8(&payload[..8]).map_err(|_| Error::Utf8Error)?;
                let size = u32::from_str_radix(hex_str, 16)
                    .map_err(|_| Error::FastbootProtocolError("invalid DATA hex length"))?;
                Ok(FastbootResponse::Data(size))
            }
            _ => Err(Error::FastbootProtocolError("unknown response prefix")),
        }
    }

    /// Sends a raw command and processes any intermediate `INFO` / `TEXT` responses via `on_info`.
    /// Returns the terminal response (`OKAY`, `FAIL`, or `DATA`).
    pub fn raw_command<T: Read + Write>(
        &self,
        transport: &mut T,
        cmd: &str,
        mut on_info: impl FnMut(&str),
    ) -> Result<FastbootResponse> {
        transport.write_all(cmd.as_bytes())?;
        transport.flush()?;

        loop {
            let resp = self.read_response(transport)?;
            match resp {
                FastbootResponse::Info(ref msg) | FastbootResponse::Text(ref msg) => {
                    on_info(msg);
                }
                _ => return Ok(resp),
            }
        }
    }

    /// Sends a command and returns the `OKAY` message string on success, or an `Error` on `FAIL`.
    pub fn command<T: Read + Write>(&self, transport: &mut T, cmd: &str) -> Result<String> {
        match self.raw_command(transport, cmd, |_| {})? {
            FastbootResponse::Okay(msg) => Ok(msg),
            FastbootResponse::Fail(reason) => Err(Error::FastbootError(reason)),
            FastbootResponse::Data(_) => Err(Error::FastbootProtocolError(
                "unexpected DATA response for standard command",
            )),
            _ => unreachable!(),
        }
    }

    /// Queries a Fastboot variable (e.g. `version`, `max-download-size`, `product`, `serialno`).
    pub fn getvar<T: Read + Write>(&self, transport: &mut T, var: &str) -> Result<String> {
        let cmd = format!("getvar:{var}");
        self.command(transport, &cmd)
    }

    /// Downloads a raw byte buffer to the device's RAM buffer.
    pub fn download<T: Read + Write>(&self, transport: &mut T, data: &[u8]) -> Result<()> {
        let cmd = format!("download:{:08x}", data.len());
        let resp = self.raw_command(transport, &cmd, |_| {})?;

        match resp {
            FastbootResponse::Data(size) => {
                if size as usize != data.len() {
                    return Err(Error::FastbootProtocolError(
                        "DATA response size does not match requested download size",
                    ));
                }
            }
            FastbootResponse::Fail(reason) => return Err(Error::FastbootError(reason)),
            FastbootResponse::Okay(_) => {
                return Err(Error::FastbootProtocolError(
                    "expected DATA response, got OKAY",
                ));
            }
            _ => unreachable!(),
        }

        transport.write_all(data)?;
        transport.flush()?;

        // Device sends OKAY / FAIL after receiving data
        match self.read_response(transport)? {
            FastbootResponse::Okay(_) => Ok(()),
            FastbootResponse::Fail(reason) => Err(Error::FastbootError(reason)),
            _ => Err(Error::FastbootProtocolError(
                "unexpected response after download data payload",
            )),
        }
    }

    /// Downloads data from a `Read` stream with a known total size.
    pub fn download_stream<T: Read + Write, R: Read>(
        &self,
        transport: &mut T,
        total_len: usize,
        reader: &mut R,
    ) -> Result<()> {
        let cmd = format!("download:{:08x}", total_len);
        let resp = self.raw_command(transport, &cmd, |_| {})?;

        match resp {
            FastbootResponse::Data(size) => {
                if size as usize != total_len {
                    return Err(Error::FastbootProtocolError(
                        "DATA response size does not match requested download size",
                    ));
                }
            }
            FastbootResponse::Fail(reason) => return Err(Error::FastbootError(reason)),
            _ => {
                return Err(Error::FastbootProtocolError(
                    "expected DATA response for download",
                ));
            }
        }

        let mut buf = [0u8; 64 * 1024];
        let mut sent = 0;
        while sent < total_len {
            let to_read = (total_len - sent).min(buf.len());
            reader.read_exact(&mut buf[..to_read])?;
            transport.write_all(&buf[..to_read])?;
            sent += to_read;
        }
        transport.flush()?;

        match self.read_response(transport)? {
            FastbootResponse::Okay(_) => Ok(()),
            FastbootResponse::Fail(reason) => Err(Error::FastbootError(reason)),
            _ => Err(Error::FastbootProtocolError(
                "unexpected response after download stream payload",
            )),
        }
    }

    /// Executes the `boot` command, instructing the device to boot the downloaded image in RAM.
    pub fn boot<T: Read + Write>(&self, transport: &mut T) -> Result<()> {
        self.command(transport, "boot").map(|_| ())
    }

    /// Flashes the downloaded data to a named partition.
    pub fn flash<T: Read + Write>(&self, transport: &mut T, partition: &str) -> Result<()> {
        let cmd = format!("flash:{partition}");
        self.command(transport, &cmd).map(|_| ())
    }

    /// Erases a named partition.
    pub fn erase<T: Read + Write>(&self, transport: &mut T, partition: &str) -> Result<()> {
        let cmd = format!("erase:{partition}");
        self.command(transport, &cmd).map(|_| ())
    }

    /// Reboots the device normally.
    pub fn reboot<T: Read + Write>(&self, transport: &mut T) -> Result<()> {
        self.command(transport, "reboot").map(|_| ())
    }

    /// Reboots the device back into bootloader/fastboot mode.
    pub fn reboot_bootloader<T: Read + Write>(&self, transport: &mut T) -> Result<()> {
        self.command(transport, "reboot-bootloader").map(|_| ())
    }

    /// Continues normal boot without flashing/rebooting.
    pub fn continue_boot<T: Read + Write>(&self, transport: &mut T) -> Result<()> {
        self.command(transport, "continue").map(|_| ())
    }
}
