#![no_std]
#![feature(core_io, alloc_io)]

extern crate alloc;

pub mod adb;
pub mod error;
pub mod fastboot;
pub mod usb;

pub use adb::{AdbAuthSigner, AdbClient, AdbDeviceInfo, Amessage};
pub use error::{Error, Result};
pub use fastboot::{FastbootClient, FastbootResponse};
pub use usb::*;

// Re-export core::io and alloc::io primitives for no_std consumers
pub mod io {
    pub use alloc::io::{
        Cursor, Error, Read, Result, Seek, SeekFrom, Take, Write, copy, empty, repeat, sink,
    };
    pub use core::io::ErrorKind;
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::io::Cursor;
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn test_fastboot_okay_response() {
        let client = FastbootClient::new();
        let mut transport = Cursor::new(b"OKAY0.5".to_vec());
        let resp = client.read_response(&mut transport).unwrap();
        assert_eq!(resp, FastbootResponse::Okay("0.5".into()));
    }

    #[test]
    fn test_fastboot_fail_response() {
        let client = FastbootClient::new();
        let mut transport = Cursor::new(b"FAILdevice is locked".to_vec());
        let resp = client.read_response(&mut transport).unwrap();
        assert_eq!(resp, FastbootResponse::Fail("device is locked".into()));
    }

    #[test]
    fn test_fastboot_data_response() {
        let client = FastbootClient::new();
        let mut transport = Cursor::new(b"DATA00010000".to_vec());
        let resp = client.read_response(&mut transport).unwrap();
        assert_eq!(resp, FastbootResponse::Data(0x10000));
    }

    #[test]
    fn test_fastboot_info_handling() {
        let client = FastbootClient::new();
        // Simulate transport delivering INFO messages followed by OKAY
        let mut transport = MockTransport::new(vec![
            b"INFOStarting boot...".to_vec(),
            b"INFOChecking signature...".to_vec(),
            b"OKAY".to_vec(),
        ]);

        let mut logs = Vec::new();
        let resp = client
            .raw_command(&mut transport, "boot", |info| {
                logs.push(info.to_string());
            })
            .unwrap();

        assert_eq!(resp, FastbootResponse::Okay("".into()));
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0], "Starting boot...");
        assert_eq!(logs[1], "Checking signature...");
    }

    #[test]
    fn test_fastboot_download_and_boot() {
        let client = FastbootClient::new();
        let payload = b"BOOT_IMAGE_DATA_12345678";
        let mut transport = MockTransport::new(vec![
            format!("DATA{:08x}", payload.len()).into_bytes(),
            b"OKAY".to_vec(), // Response after receiving payload
            b"OKAY".to_vec(), // Response after 'boot' command
        ]);

        client.download(&mut transport, payload).unwrap();
        client.boot(&mut transport).unwrap();

        // Verify sent data
        let sent = transport.written_data();
        let expected_cmd = format!("download:{:08x}", payload.len());
        assert!(sent.starts_with(expected_cmd.as_bytes()));
    }

    #[test]
    fn test_adb_message_encode_decode() {
        let payload = b"host::features=cmd\0";
        let msg = Amessage::new(adb::A_CNXN, adb::A_VERSION, adb::A_MAXDATA, payload);

        let bytes = msg.to_bytes();
        let decoded = Amessage::from_bytes(&bytes).unwrap();

        assert_eq!(msg, decoded);
        assert_eq!(decoded.command, adb::A_CNXN);
        assert_eq!(decoded.arg0, adb::A_VERSION);
        assert_eq!(decoded.arg1, adb::A_MAXDATA);
        assert_eq!(decoded.data_length, payload.len() as u32);
        assert_eq!(decoded.data_check, adb::calculate_checksum(payload));
        assert_eq!(decoded.magic, adb::A_CNXN ^ 0xFFFFFFFF);
    }

    #[test]
    fn test_adb_checksum() {
        let data = b"reboot:bootloader\0";
        let expected: u32 = data.iter().map(|&b| b as u32).sum();
        assert_eq!(adb::calculate_checksum(data), expected);
    }

    #[test]
    fn test_adb_connect_and_reboot_bootloader() {
        let client = AdbClient::new();
        let device_banner = b"device::ro.product.name=test;ro.product.model=test;\0";
        let cnxn_resp = Amessage::new(adb::A_CNXN, adb::A_VERSION, 4096, device_banner);
        let okay_resp = Amessage::new(adb::A_OKAY, 42, 1, &[]);

        let mut resp_stream = Vec::new();
        resp_stream.extend_from_slice(&cnxn_resp.to_bytes());
        resp_stream.extend_from_slice(device_banner);
        resp_stream.extend_from_slice(&okay_resp.to_bytes());

        let mut transport = MockStream::new(resp_stream);

        let dev_info = client.connect(&mut transport, "host::").unwrap();
        assert_eq!(dev_info.version, adb::A_VERSION);
        assert_eq!(dev_info.max_payload, 4096);

        client.reboot_bootloader(&mut transport).unwrap();
    }

    struct MockStream {
        read_data: Vec<u8>,
        read_pos: usize,
        written: Vec<u8>,
    }

    impl MockStream {
        fn new(read_data: Vec<u8>) -> Self {
            Self {
                read_data,
                read_pos: 0,
                written: Vec::new(),
            }
        }
    }

    impl alloc::io::Read for MockStream {
        fn read(&mut self, buf: &mut [u8]) -> alloc::io::Result<usize> {
            if self.read_pos >= self.read_data.len() {
                return Ok(0);
            }
            let avail = self.read_data.len() - self.read_pos;
            let n = avail.min(buf.len());
            buf[..n].copy_from_slice(&self.read_data[self.read_pos..self.read_pos + n]);
            self.read_pos += n;
            Ok(n)
        }
    }

    impl alloc::io::Write for MockStream {
        fn write(&mut self, buf: &[u8]) -> alloc::io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> alloc::io::Result<()> {
            Ok(())
        }
    }

    struct MockTransport {
        packets: Vec<Vec<u8>>,
        packet_idx: usize,
        written: Vec<u8>,
    }

    impl MockTransport {
        fn new(packets: Vec<Vec<u8>>) -> Self {
            Self {
                packets,
                packet_idx: 0,
                written: Vec::new(),
            }
        }

        fn written_data(&self) -> &[u8] {
            &self.written
        }
    }

    impl alloc::io::Read for MockTransport {
        fn read(&mut self, buf: &mut [u8]) -> alloc::io::Result<usize> {
            if self.packet_idx >= self.packets.len() {
                return Ok(0);
            }
            let pkt = &self.packets[self.packet_idx];
            let n = pkt.len().min(buf.len());
            buf[..n].copy_from_slice(&pkt[..n]);
            self.packet_idx += 1;
            Ok(n)
        }
    }

    impl alloc::io::Write for MockTransport {
        fn write(&mut self, buf: &[u8]) -> alloc::io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> alloc::io::Result<()> {
            Ok(())
        }
    }
}
