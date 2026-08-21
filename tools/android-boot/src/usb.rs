//! USB transport for Fastboot and ADB using `nusb`.

#![allow(unused)]

extern crate alloc;

use alloc::io::{Read, Write};
use android_boot_protocol::usb::{
    ADB_CLASS, ADB_PROTOCOL, ADB_SUBCLASS, FASTBOOT_CLASS, FASTBOOT_PROTOCOL, FASTBOOT_SUBCLASS,
    GOOGLE_USB_VID, is_adb_interface, is_fastboot_interface,
};
use nusb::descriptors::TransferType;
use nusb::transfer::{Buffer, Bulk, In, Out};
use nusb::{DeviceInfo, Endpoint, Interface, MaybeFuture};
use rootcause::{Result, bail, report};
use std::time::{Duration, Instant};

pub struct UsbTransport {
    _interface: Interface,
    out_ep: Endpoint<Bulk, Out>,
    in_ep: Endpoint<Bulk, In>,
    read_buf: Vec<u8>,
    read_pos: usize,
    timeout: Duration,
}

impl UsbTransport {
    /// Opens an Android Fastboot USB device.
    pub fn open_fastboot(serial: Option<&str>, timeout: Duration) -> Result<Self> {
        Self::open_device(
            |class, sub, proto, vid| {
                is_fastboot_interface(class, sub, proto) || (vid == GOOGLE_USB_VID && sub == 0x42)
            },
            serial,
            timeout,
            "Fastboot",
        )
    }

    /// Opens an Android ADB USB device.
    pub fn open_adb(serial: Option<&str>, timeout: Duration) -> Result<Self> {
        Self::open_device(
            |class, sub, proto, _| is_adb_interface(class, sub, proto),
            serial,
            timeout,
            "ADB",
        )
    }

    fn open_device(
        matcher: impl Fn(u8, u8, u8, u16) -> bool,
        serial: Option<&str>,
        timeout: Duration,
        protocol_name: &'static str,
    ) -> Result<Self> {
        let start = Instant::now();
        loop {
            let devices = nusb::list_devices()
                .wait()
                .map_err(|e| report!("failed to enumerate USB devices: {e}"))?;

            for info in devices {
                if let Some(s) = serial {
                    if info.serial_number() != Some(s) {
                        continue;
                    }
                }

                if let Some(transport) = Self::try_open_matched(&info, &matcher) {
                    return Ok(transport);
                }
            }

            if start.elapsed() >= timeout {
                if let Some(s) = serial {
                    bail!("timed out waiting for {protocol_name} USB device with serial '{s}'");
                } else {
                    bail!("timed out waiting for {protocol_name} USB device");
                }
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn try_open_matched(
        info: &DeviceInfo,
        matcher: &impl Fn(u8, u8, u8, u16) -> bool,
    ) -> Option<Self> {
        let device = info.open().wait().ok()?;

        // On macOS, explicitly set configuration 1 if no active configuration exists
        if device.active_configuration().is_err() {
            let _ = device.set_configuration(1).wait();
        }

        let mut matched_interface_num = None;

        for config in device.configurations() {
            for iface in config.interfaces() {
                for alt in iface.alt_settings() {
                    if matcher(
                        alt.class(),
                        alt.subclass(),
                        alt.protocol(),
                        info.vendor_id(),
                    ) {
                        matched_interface_num = Some(iface.interface_number());
                        break;
                    }
                }
                if matched_interface_num.is_some() {
                    break;
                }
            }
            if matched_interface_num.is_some() {
                break;
            }
        }

        // If interface match wasn't found by class/subclass, fall back to interface 0
        let iface_num = matched_interface_num.unwrap_or(0);

        let interface = device.claim_interface(iface_num).wait().ok()?;
        let (out_ep_addr, in_ep_addr) = probe_bulk_endpoints(&interface)?;

        let out_ep = interface.endpoint::<Bulk, Out>(out_ep_addr).ok()?;
        let in_ep = interface.endpoint::<Bulk, In>(in_ep_addr).ok()?;

        Some(Self {
            _interface: interface,
            out_ep,
            in_ep,
            read_buf: Vec::new(),
            read_pos: 0,
            timeout: Duration::from_secs(5),
        })
    }
}

fn probe_bulk_endpoints(interface: &Interface) -> Option<(u8, u8)> {
    let desc = interface.descriptor()?;
    let mut out_ep = None;
    let mut in_ep = None;
    for ep in desc.endpoints() {
        if ep.transfer_type() != TransferType::Bulk {
            continue;
        }
        if ep.address() & 0x80 != 0 {
            in_ep = Some(ep.address());
        } else {
            out_ep = Some(ep.address());
        }
    }
    match (out_ep, in_ep) {
        (Some(o), Some(i)) => Some((o, i)),
        _ => None,
    }
}

impl Read for UsbTransport {
    fn read(&mut self, buf: &mut [u8]) -> alloc::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // Serve from buffered data first
        if self.read_pos < self.read_buf.len() {
            let avail = self.read_buf.len() - self.read_pos;
            let to_copy = avail.min(buf.len());
            buf[..to_copy].copy_from_slice(&self.read_buf[self.read_pos..self.read_pos + to_copy]);
            self.read_pos += to_copy;
            if self.read_pos >= self.read_buf.len() {
                self.read_buf.clear();
                self.read_pos = 0;
            }
            return Ok(to_copy);
        }

        let max_packet = self.in_ep.max_packet_size().max(512);
        let requested = buf.len().max(max_packet).div_ceil(max_packet) * max_packet;

        let completion = self
            .in_ep
            .transfer_blocking(Buffer::new(requested), self.timeout);
        let received = completion.into_result().map_err(|e| {
            alloc::io::Error::new(
                alloc::io::ErrorKind::TimedOut,
                alloc::format!("USB bulk read failed: {e}"),
            )
        })?;

        if received.is_empty() {
            return Ok(0);
        }

        let to_copy = received.len().min(buf.len());
        buf[..to_copy].copy_from_slice(&received[..to_copy]);

        if received.len() > to_copy {
            self.read_buf.clear();
            self.read_buf.extend_from_slice(&received[to_copy..]);
            self.read_pos = 0;
        }

        Ok(to_copy)
    }
}

impl Write for UsbTransport {
    fn write(&mut self, buf: &[u8]) -> alloc::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let chunk_size = buf.len().min(1024 * 1024);
        let completion = self
            .out_ep
            .transfer_blocking(buf[..chunk_size].to_vec().into(), self.timeout);
        let _ = completion.into_result().map_err(|e| {
            alloc::io::Error::new(
                alloc::io::ErrorKind::TimedOut,
                alloc::format!("USB bulk write failed: {e}"),
            )
        })?;

        Ok(chunk_size)
    }

    fn flush(&mut self) -> alloc::io::Result<()> {
        Ok(())
    }
}
