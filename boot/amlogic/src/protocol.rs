//! Amlogic USB boot protocol implementation.
//!
//! Ported from `pyamlboot` to native Rust:
//! - Memory read/write (small and large chunk transfers).
//! - Identification and SocId query.
//! - U-Boot `bulkCmd` execution for RAM-boot and diagnostics.

use super::socid::SocId;
use super::usb::UsbDevice;
use rootcause::{bail, report, Result};
use std::time::Duration;

pub const REQ_WRITE_MEM: u8 = 0x01;
pub const REQ_READ_MEM: u8 = 0x02;
pub const REQ_FILL_MEM: u8 = 0x03;
pub const REQ_MODIFY_MEM: u8 = 0x04;
pub const REQ_RUN_IN_ADDR: u8 = 0x05;
pub const REQ_WR_LARGE_MEM: u8 = 0x11;
pub const REQ_RD_LARGE_MEM: u8 = 0x12;
pub const REQ_IDENTIFY_HOST: u8 = 0x20;
pub const REQ_TPL_CMD: u8 = 0x30;
pub const REQ_TPL_STAT: u8 = 0x31;
pub const REQ_BULKCMD: u8 = 0x34;
pub const REQ_PASSWORD: u8 = 0x35;
pub const REQ_NOP: u8 = 0x36;
pub const REQ_GET_AMLC: u8 = 0x50;
pub const REQ_WRITE_AMLC: u8 = 0x60;

pub const AMLC_AMLS_BLOCK_LENGTH: usize = 0x200;
pub const AMLC_MAX_BLOCK_LENGTH: usize = 0x4000;
pub const AMLC_MAX_TRANSFER_LENGTH: usize = 65536;

pub const FLAG_KEEP_POWER_ON: u32 = 0x10;
pub const MAX_LARGE_BLOCK_COUNT: usize = 65535;

pub struct AmlogicDevice {
    dev: UsbDevice,
}

impl AmlogicDevice {
    /// Opens the Amlogic SoC connected in USB Boot / Burn Mode (VID: 0x1b8e, PID: 0xc003).
    pub fn open(timeout: Duration) -> Result<Self> {
        let dev = UsbDevice::open(0x1b8e, 0xc003, timeout)?;
        Ok(Self { dev })
    }

    /// Queries the ROM/Stage identity of the connected device.
    pub fn identify(&self) -> Result<SocId> {
        let mut buf = [0u8; 8];
        self.dev
            .control_transfer(0xc0, REQ_IDENTIFY_HOST, 0, 0, &mut buf, 1000)
            .map_err(|e| report!("identify failed: {}", e))?;
        SocId::from_bytes(&buf)
    }

    /// Sends a NOP command.
    pub fn nop(&self) -> Result<()> {
        let mut buf = [];
        self.dev
            .control_transfer(0x40, REQ_NOP, 0, 0, &mut buf, 1000)
            .map_err(|e| report!("nop failed: {}", e))?;
        Ok(())
    }

    /// Writes a small chunk of data (up to 64 bytes) to memory.
    pub fn write_simple_memory(&self, address: u32, data: &[u8]) -> Result<()> {
        if data.len() > 64 {
            bail!("write_simple_memory: maximum size is 64 bytes");
        }
        let mut buf = data.to_vec();
        self.dev
            .control_transfer(
                0x40,
                REQ_WRITE_MEM,
                (address >> 16) as u16,
                (address & 0xffff) as u16,
                &mut buf,
                1000,
            )
            .map_err(|e| report!("write_simple_memory at {:#x} failed: {}", address, e))?;
        Ok(())
    }

    /// Writes arbitrary data to memory via 64-byte control transfers.
    pub fn write_memory(&self, address: u32, data: &[u8]) -> Result<()> {
        let mut offset = 0;
        while offset < data.len() {
            let chunk_len = (data.len() - offset).min(64);
            self.write_simple_memory(address + offset as u32, &data[offset..offset + chunk_len])?;
            offset += chunk_len;
        }
        Ok(())
    }

    /// Reads a small chunk of data (up to 64 bytes) from memory.
    pub fn read_simple_memory(&self, address: u32, length: usize) -> Result<Vec<u8>> {
        if length == 0 {
            return Ok(Vec::new());
        }
        if length > 64 {
            bail!("read_simple_memory: maximum size is 64 bytes");
        }
        let mut buf = vec![0u8; length];
        self.dev
            .control_transfer(
                0xc0,
                REQ_READ_MEM,
                (address >> 16) as u16,
                (address & 0xffff) as u16,
                &mut buf,
                1000,
            )
            .map_err(|e| report!("read_simple_memory at {:#x} failed: {}", address, e))?;
        Ok(buf)
    }

    /// Reads arbitrary memory via 64-byte control transfers.
    pub fn read_memory(&self, address: u32, length: usize) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(length);
        let mut offset = 0;
        while offset < length {
            let chunk_len = (length - offset).min(64);
            let chunk = self.read_simple_memory(address + offset as u32, chunk_len)?;
            data.extend_from_slice(&chunk);
            offset += chunk_len;
        }
        Ok(data)
    }

    /// Internal single large memory transfer.
    fn write_large_memory_chunk(
        &self,
        address: u32,
        data: &[u8],
        block_length: usize,
    ) -> Result<()> {
        let mut padded_data = data.to_vec();
        if padded_data.len() % block_length != 0 {
            let rem = block_length - (padded_data.len() % block_length);
            padded_data.extend(core::iter::repeat(0).take(rem));
        }

        let block_count = padded_data.len() / block_length;

        // Control header: address, length, 0, 0 in little-endian
        let mut control_data = [0u8; 16];
        control_data[0..4].copy_from_slice(&address.to_le_bytes());
        control_data[4..8].copy_from_slice(&(padded_data.len() as u32).to_le_bytes());

        self.dev
            .control_transfer(
                0x40,
                REQ_WR_LARGE_MEM,
                block_length as u16,
                block_count as u16,
                &mut control_data,
                1000,
            )
            .map_err(|e| report!("write_large_memory control transfer failed: {}", e))?;

        // Bulk stream
        let mut offset = 0;
        while offset < padded_data.len() {
            let chunk_end = (offset + block_length).min(padded_data.len());
            self.dev
                .bulk_write(self.dev.out_ep, &padded_data[offset..chunk_end], 2000)
                .map_err(|e| report!("bulk_write at offset {} failed: {}", offset, e))?;
            offset += block_length;
        }

        Ok(())
    }

    /// Writes large buffers (e.g. kernel Image or DTB) to DRAM with high throughput.
    pub fn write_large_memory<F>(
        &self,
        address: u32,
        data: &[u8],
        block_length: usize,
        mut progress: Option<F>,
    ) -> Result<()>
    where
        F: FnMut(usize, usize),
    {
        let total = data.len();
        let max_transfer_bytes = MAX_LARGE_BLOCK_COUNT * block_length;
        let mut offset = 0;

        while offset < total {
            let chunk_len = (total - offset).min(max_transfer_bytes);
            self.write_large_memory_chunk(
                address + offset as u32,
                &data[offset..offset + chunk_len],
                block_length,
            )?;
            offset += chunk_len;
            if let Some(p) = &mut progress {
                p(offset.min(total), total);
            }
        }

        Ok(())
    }

    /// Executes code at the specified DRAM address.
    pub fn run(&self, address: u32, keep_power: bool) -> Result<()> {
        let val = if keep_power {
            address | FLAG_KEEP_POWER_ON
        } else {
            address
        };
        let mut control_data = val.to_le_bytes();
        self.dev
            .control_transfer(
                0x40,
                REQ_RUN_IN_ADDR,
                (address >> 16) as u16,
                (address & 0xffff) as u16,
                &mut control_data,
                1000,
            )
            .map_err(|e| report!("run at {:#x} failed: {}", address, e))?;
        Ok(())
    }

    /// Sends a text command to U-Boot over USB (e.g. `booti 0x02000000 - 0x04000000` or `fastboot`).
    pub fn bulk_cmd(&self, command: &str) -> Result<Option<String>> {
        let mut cmd_bytes = command.as_bytes().to_vec();
        cmd_bytes.push(0); // Null terminator

        if cmd_bytes.len() >= 128 {
            bail!("Bulk command exceeds 127 characters limit");
        }

        self.dev
            .control_transfer(0x40, REQ_BULKCMD, 0, 2, &mut cmd_bytes, 2000)
            .map_err(|e| report!("bulk_cmd '{}' failed: {}", command, e))?;

        let mut reply = [0u8; 512];
        match self.dev.bulk_read(self.dev.in_ep, &mut reply, 1000) {
            Ok(n) => {
                let text = String::from_utf8_lossy(&reply[..n]).to_string();
                Ok(Some(text))
            }
            Err(_) => Ok(None),
        }
    }

    /// Reads BL2 Boot AMLC data request in MaskROM mode.
    pub fn get_boot_amlc(&self) -> Result<(usize, usize)> {
        let mut buf = [];
        self.dev
            .control_transfer(
                0x40,
                REQ_GET_AMLC,
                AMLC_AMLS_BLOCK_LENGTH as u16,
                0,
                &mut buf,
                1000,
            )
            .map_err(|e| report!("REQ_GET_AMLC control transfer failed: {}", e))?;

        let mut resp = [0u8; AMLC_AMLS_BLOCK_LENGTH];
        let n = self
            .dev
            .bulk_read(self.dev.in_ep, &mut resp, 1000)
            .map_err(|e| report!("Reading AMLC request failed: {}", e))?;

        if n < 16 || &resp[0..4] != b"AMLC" {
            bail!("Invalid AMLC request response: {:?}", &resp[0..n.min(16)]);
        }

        let length = u32::from_le_bytes([resp[8], resp[9], resp[10], resp[11]]) as usize;
        let offset = u32::from_le_bytes([resp[12], resp[13], resp[14], resp[15]]) as usize;

        // Acknowledge the request with OKAY packet
        let mut okay = [0u8; 16];
        okay[0..4].copy_from_slice(b"OKAY");
        self.dev
            .bulk_write(self.dev.out_ep, &okay, 1000)
            .map_err(|e| report!("Sending AMLC OKAY ack failed: {}", e))?;

        Ok((length, offset))
    }

    fn _write_amlc_chunk(&self, offset: usize, data: &[u8]) -> Result<()> {
        let write_len = data.len();
        let block_count = (write_len + AMLC_MAX_BLOCK_LENGTH - 1) / AMLC_MAX_BLOCK_LENGTH;
        let mut ctrl_buf = [];

        self.dev
            .control_transfer(
                0x40,
                REQ_WRITE_AMLC,
                (offset / AMLC_AMLS_BLOCK_LENGTH) as u16,
                (write_len - 1) as u16,
                &mut ctrl_buf,
                1000,
            )
            .map_err(|e| report!("REQ_WRITE_AMLC control transfer failed: {}", e))?;

        let mut data_offset = 0;
        for _ in 0..block_count {
            let remain = write_len - data_offset;
            let chunk_len = remain.min(AMLC_MAX_BLOCK_LENGTH);
            self.dev
                .bulk_write(
                    self.dev.out_ep,
                    &data[data_offset..data_offset + chunk_len],
                    1000,
                )
                .map_err(|e| report!("Writing AMLC block failed: {}", e))?;
            data_offset += chunk_len;
        }

        // Read 16-byte ACK
        let mut ack = [0u8; 16];
        let _ = self.dev.bulk_read(self.dev.in_ep, &mut ack, 1000);
        if &ack[0..4] != b"OKAY" {
            bail!("Invalid AMLC write ACK: {:?}", &ack);
        }

        Ok(())
    }

    /// Writes requested AMLC payload and signs with AMLS checksum footer.
    pub fn write_amlc_data(&self, seq: u8, amlc_offset: usize, data: &[u8]) -> Result<()> {
        let data_len = data.len();
        let transfer_count = (data_len + AMLC_MAX_TRANSFER_LENGTH - 1) / AMLC_MAX_TRANSFER_LENGTH;
        let mut offset = 0;

        for _ in 0..transfer_count {
            let remain = data_len - offset;
            let write_len = remain.min(AMLC_MAX_TRANSFER_LENGTH);
            self._write_amlc_chunk(offset, &data[offset..offset + write_len])?;
            offset += write_len;
        }

        // Write AMLS packet with checksum
        let checksum = amls_checksum(data);
        let mut amls = [0u8; 512];
        amls[0..4].copy_from_slice(b"AMLS");
        amls[4] = seq;
        amls[8..12].copy_from_slice(&checksum.to_le_bytes());
        if data.len() >= 512 {
            amls[16..512].copy_from_slice(&data[16..512]);
        } else if data.len() > 16 {
            amls[16..data.len()].copy_from_slice(&data[16..]);
        }

        self._write_amlc_chunk(amlc_offset, &amls)
    }

    /// Boots a G12A device from MaskROM mode (Stage 0.16) into U-Boot/TPL (Stage 0.2).
    pub fn boot_g12(&self, uboot_data: &[u8]) -> Result<()> {
        let load_addr = 0xfffa_0000u32;
        let first_chunk_len = uboot_data.len().min(0x10000);

        println!(
            "==> Writing initial BL2 stage ({} bytes) to SRAM at {:#x}...",
            first_chunk_len, load_addr
        );
        self.write_large_memory(
            load_addr,
            &uboot_data[0..first_chunk_len],
            4096,
            None::<fn(usize, usize)>,
        )?;

        println!("==> Executing BL2 at {:#x}...", load_addr);
        self.run(load_addr, true)?;

        std::thread::sleep(Duration::from_millis(1500));

        let mut seq = 0u8;
        let mut prev_len = usize::MAX;
        let mut prev_off = usize::MAX;

        loop {
            match self.get_boot_amlc() {
                Ok((length, offset)) => {
                    if length == prev_len && offset == prev_off {
                        println!("==> BL2 loading complete! U-Boot running in DRAM.");
                        break;
                    }
                    prev_len = length;
                    prev_off = offset;

                    if offset + length > uboot_data.len() {
                        bail!("AMLC request exceeds bootloader binary bounds");
                    }

                    println!(
                        "    AMLC request: size={} bytes, offset={:#x}, seq={}",
                        length, offset, seq
                    );
                    self.write_amlc_data(seq, offset, &uboot_data[offset..offset + length])?;
                    seq = seq.wrapping_add(1);
                }
                Err(e) => {
                    println!("==> Finished AMLC sequence ({})", e);
                    break;
                }
            }
        }

        Ok(())
    }
}

fn amls_checksum(data: &[u8]) -> u32 {
    let mut checksum = 0u32;
    let mut offset = 0;
    while offset < data.len() {
        let left = data.len() - offset;
        let val = if left >= 4 {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        } else if left == 3 {
            u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], 0])
        } else if left == 2 {
            u16::from_le_bytes([data[offset], data[offset + 1]]) as u32
        } else {
            data[offset] as u32
        };
        offset += 4;
        checksum = checksum.wrapping_add(val);
    }
    checksum
}
