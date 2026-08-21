//! ADB (Android Debug Bridge) client protocol implementation (`no_std`).
//!
//! Implements ADB framing, connection handshake (`A_CNXN`), and service invocation (`A_OPEN`):
//! - 24-byte message header encode/decode and validation (checksum + magic verification)
//! - Connection establishment
//! - Service opening for reboot commands (`reboot:bootloader`, `reboot:`, etc.)

use crate::error::{Error, Result};
use alloc::format;
use alloc::io::{Read, Write};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

pub const A_SYNC: u32 = 0x434e5953;
pub const A_CNXN: u32 = 0x4e584e43;
pub const A_OPEN: u32 = 0x4e45504f;
pub const A_OKAY: u32 = 0x59414b4f;
pub const A_CLSE: u32 = 0x45534c43;
pub const A_WRTE: u32 = 0x45545257;
pub const A_AUTH: u32 = 0x48545541;
pub const A_STLS: u32 = 0x534c5453;

pub const A_VERSION_MIN: u32 = 0x01000000;
pub const A_VERSION: u32 = 0x01000000;
pub const A_MAXDATA: u32 = 256 * 1024;

pub const AUTH_TYPE_TOKEN: u32 = 1;
pub const AUTH_TYPE_SIGNATURE: u32 = 2;
pub const AUTH_TYPE_RSAPUBLICKEY: u32 = 3;

/// Computes the ADB checksum (sum of all byte values as a 32-bit integer).
pub fn calculate_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    for &b in data {
        sum = sum.wrapping_add(b as u32);
    }
    sum
}

/// 24-byte ADB message header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Amessage {
    /// Command identifier constant (`A_CNXN`, `A_OPEN`, `A_OKAY`, etc.)
    pub command: u32,
    /// First argument (e.g. version, local-id, or remote-id depending on command)
    pub arg0: u32,
    /// Second argument (e.g. maxdata, remote-id, etc.)
    pub arg1: u32,
    /// Length of the payload in bytes (0 if none)
    pub data_length: u32,
    /// Checksum of the payload bytes
    pub data_check: u32,
    /// Magic integrity check (`command ^ 0xFFFFFFFF`)
    pub magic: u32,
}

impl Amessage {
    /// Constructs a new message header with computed checksum and magic.
    pub fn new(command: u32, arg0: u32, arg1: u32, data: &[u8]) -> Self {
        Self {
            command,
            arg0,
            arg1,
            data_length: data.len() as u32,
            data_check: calculate_checksum(data),
            magic: command ^ 0xFFFFFFFF,
        }
    }

    /// Serializes the header to a 24-byte little-endian array.
    pub fn to_bytes(&self) -> [u8; 24] {
        let mut buf = [0u8; 24];
        buf[0..4].copy_from_slice(&self.command.to_le_bytes());
        buf[4..8].copy_from_slice(&self.arg0.to_le_bytes());
        buf[8..12].copy_from_slice(&self.arg1.to_le_bytes());
        buf[12..16].copy_from_slice(&self.data_length.to_le_bytes());
        buf[16..20].copy_from_slice(&self.data_check.to_le_bytes());
        buf[20..24].copy_from_slice(&self.magic.to_le_bytes());
        buf
    }

    /// Deserializes and validates a 24-byte little-endian header.
    pub fn from_bytes(buf: &[u8; 24]) -> Result<Self> {
        let command = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let arg0 = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let arg1 = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let data_length = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let data_check = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let magic = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);

        let expected_magic = command ^ 0xFFFFFFFF;
        if magic != expected_magic {
            return Err(Error::InvalidMagic {
                expected: expected_magic,
                actual: magic,
            });
        }

        Ok(Self {
            command,
            arg0,
            arg1,
            data_length,
            data_check,
            magic,
        })
    }
}

/// Information about a connected ADB device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbDeviceInfo {
    /// ADB protocol version reported by device
    pub version: u32,
    /// Maximum payload size supported by device
    pub max_payload: u32,
    /// Identification banner / properties string reported by device
    pub banner: String,
}

/// Provides RSA authentication operations for the ADB `A_AUTH` challenge/response
/// handshake, decoupling the wire protocol (`adb.rs`, `no_std`) from any
/// particular crypto backend or key-storage mechanism (left to the caller,
/// e.g. a `std`-based CLI tool using RustCrypto's `rsa` crate).
pub trait AdbAuthSigner {
    /// Signs `token` (the device's random 20-byte SHA1 challenge) with an
    /// available private key, returning the raw PKCS#1 v1.5 (SHA1) signature
    /// bytes. Returns `Ok(None)` if no (further) private key is available to
    /// try, causing the caller to fall back to public-key registration.
    fn sign(&self, token: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Returns the ASCII payload to send in an `A_AUTH` `RSAPUBLICKEY` packet
    /// (base64-encoded Android public key struct, `" user@host"` suffix), or
    /// `None` if no public key is available to offer.
    fn public_key(&self) -> Option<&[u8]>;
}

/// ADB client for managing device connections and opening service channels.
#[derive(Debug, Default, Clone, Copy)]
pub struct AdbClient;

impl AdbClient {
    /// Creates a new `AdbClient`.
    pub const fn new() -> Self {
        Self
    }

    /// Sends an ADB packet (24-byte header followed by data payload).
    pub fn send_packet<T: Write>(
        &self,
        transport: &mut T,
        msg: &Amessage,
        data: &[u8],
    ) -> Result<()> {
        let header = msg.to_bytes();
        transport.write_all(&header)?;
        if !data.is_empty() {
            transport.write_all(data)?;
        }
        transport.flush()?;
        Ok(())
    }

    /// Reads an ADB packet (24-byte header followed by data payload).
    pub fn read_packet<T: Read>(&self, transport: &mut T) -> Result<(Amessage, Vec<u8>)> {
        let mut header_buf = [0u8; 24];
        transport.read_exact(&mut header_buf)?;
        let msg = Amessage::from_bytes(&header_buf)?;

        let mut data = vec![0u8; msg.data_length as usize];
        if msg.data_length > 0 {
            transport.read_exact(&mut data)?;
            let actual_check = calculate_checksum(&data);
            if actual_check != msg.data_check {
                return Err(Error::ChecksumMismatch {
                    expected: msg.data_check,
                    actual: actual_check,
                });
            }
        }

        Ok((msg, data))
    }

    /// Performs the initial ADB connection handshake (`A_CNXN`).
    pub fn connect<T: Read + Write>(
        &self,
        transport: &mut T,
        banner: &str,
    ) -> Result<AdbDeviceInfo> {
        let mut host_banner = banner.as_bytes().to_vec();
        if !host_banner.ends_with(&[0]) {
            host_banner.push(0);
        }

        let msg = Amessage::new(A_CNXN, A_VERSION, A_MAXDATA, &host_banner);
        self.send_packet(transport, &msg, &host_banner)?;

        let (resp_msg, resp_data) = self.read_packet(transport)?;

        if resp_msg.command == A_AUTH {
            return Err(Error::AdbAuthRequired);
        }

        if resp_msg.command != A_CNXN {
            return Err(Error::AdbProtocolError(
                "expected A_CNXN response during handshake",
            ));
        }

        let banner_str = String::from_utf8_lossy(&resp_data)
            .trim_end_matches('\0')
            .to_string();

        Ok(AdbDeviceInfo {
            version: resp_msg.arg0,
            max_payload: resp_msg.arg1,
            banner: banner_str,
        })
    }

    /// Performs the ADB connection handshake, transparently handling RSA
    /// challenge/response authentication (`A_AUTH`) if the device requests it.
    ///
    /// `signer` provides the private-key signing operation and (optionally) a
    /// public key to register with the device the first time it's seen. This
    /// mirrors the real `adb` client flow: try the available private key by
    /// signing the device's random token; if the device keeps rejecting
    /// signatures (i.e. sends another `A_AUTH` TOKEN instead of `A_CNXN`),
    /// fall back to offering the public key and wait for the user to accept
    /// the on-device "Allow USB debugging?" authorization prompt.
    pub fn connect_with_auth<T: Read + Write, S: AdbAuthSigner>(
        &self,
        transport: &mut T,
        banner: &str,
        signer: &S,
    ) -> Result<AdbDeviceInfo> {
        let mut host_banner = banner.as_bytes().to_vec();
        if !host_banner.ends_with(&[0]) {
            host_banner.push(0);
        }

        let msg = Amessage::new(A_CNXN, A_VERSION, A_MAXDATA, &host_banner);
        self.send_packet(transport, &msg, &host_banner)?;

        let mut sent_pubkey = false;

        loop {
            let (resp_msg, resp_data) = self.read_packet(transport)?;

            match resp_msg.command {
                A_CNXN => {
                    let banner_str = String::from_utf8_lossy(&resp_data)
                        .trim_end_matches('\0')
                        .to_string();
                    return Ok(AdbDeviceInfo {
                        version: resp_msg.arg0,
                        max_payload: resp_msg.arg1,
                        banner: banner_str,
                    });
                }
                A_AUTH if resp_msg.arg0 == AUTH_TYPE_TOKEN => {
                    if let Some(signature) = signer.sign(&resp_data)? {
                        let sig_msg = Amessage::new(A_AUTH, AUTH_TYPE_SIGNATURE, 0, &signature);
                        self.send_packet(transport, &sig_msg, &signature)?;
                        continue;
                    }

                    if sent_pubkey {
                        return Err(Error::AdbAuthRequired);
                    }

                    match signer.public_key() {
                        Some(pubkey) => {
                            let mut payload = pubkey.to_vec();
                            if !payload.ends_with(&[0]) {
                                payload.push(0);
                            }
                            let key_msg =
                                Amessage::new(A_AUTH, AUTH_TYPE_RSAPUBLICKEY, 0, &payload);
                            self.send_packet(transport, &key_msg, &payload)?;
                            sent_pubkey = true;
                        }
                        None => return Err(Error::AdbAuthRequired),
                    }
                }
                _ => {
                    return Err(Error::AdbProtocolError(
                        "unexpected response during authenticated handshake",
                    ));
                }
            }
        }
    }

    /// Opens a named service on the device (`A_OPEN`).
    /// Returns the assigned remote ID on success.
    pub fn open_service<T: Read + Write>(
        &self,
        transport: &mut T,
        local_id: u32,
        service: &str,
    ) -> Result<u32> {
        let mut payload = service.as_bytes().to_vec();
        if !payload.ends_with(&[0]) {
            payload.push(0);
        }

        let msg = Amessage::new(A_OPEN, local_id, 0, &payload);
        self.send_packet(transport, &msg, &payload)?;

        let (resp_msg, resp_data) = self.read_packet(transport)?;

        match resp_msg.command {
            A_OKAY => {
                let remote_id = resp_msg.arg0;
                Ok(remote_id)
            }
            A_CLSE => {
                let reason = String::from_utf8_lossy(&resp_data).to_string();
                Err(Error::AdbRejected(reason))
            }
            _ => Err(Error::AdbProtocolError(
                "unexpected response to A_OPEN request",
            )),
        }
    }

    /// Instructs the device to reboot to the specified target (e.g. `"bootloader"`, `"recovery"`, or `""`).
    pub fn reboot<T: Read + Write>(&self, transport: &mut T, target: &str) -> Result<()> {
        let service = if target.is_empty() {
            "reboot:".to_string()
        } else {
            format!("reboot:{target}")
        };

        // Note: Some devices execute the reboot immediately upon receiving A_OPEN
        // and may either reply with A_OKAY or drop the USB link immediately.
        // If an I/O error occurs after sending A_OPEN, it is often due to the reboot reset.
        match self.open_service(transport, 1, &service) {
            Ok(_) => Ok(()),
            Err(Error::Io(_)) => {
                // USB link reset during reboot is common and expected
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Instructs the device to reboot directly into bootloader mode (`reboot:bootloader`).
    pub fn reboot_bootloader<T: Read + Write>(&self, transport: &mut T) -> Result<()> {
        self.reboot(transport, "bootloader")
    }

    /// Closes an open service stream (`A_CLSE`).
    pub fn close_service<T: Read + Write>(
        &self,
        transport: &mut T,
        local_id: u32,
        remote_id: u32,
    ) -> Result<()> {
        let msg = Amessage::new(A_CLSE, local_id, remote_id, &[]);
        self.send_packet(transport, &msg, &[])
    }
}
