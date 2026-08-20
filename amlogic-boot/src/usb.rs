//! Cross-platform USB bulk-boot transport for Amlogic devices.
//!
//! The Linux-usbfs-ioctl version this replaced only worked on Linux
//! (`/dev/bus/usb/*` + `USBDEVFS_*` ioctls don't exist anywhere else), which
//! is a hard blocker on macOS. Rebuilt on `nusb` - pure Rust, no libusb C
//! dependency (matching this file's original "no external C library"
//! design goal), backed by IOKit on macOS / usbfs on Linux / WinUSB on
//! Windows. `protocol.rs` is unaware of the swap: this module keeps the
//! same blocking `control_transfer`/`bulk_write`/`bulk_read` surface.

use parking_lot::Mutex;
use std::time::{Duration, Instant};

use nusb::transfer::{Buffer, Bulk, ControlIn, ControlOut, ControlType, In, Out, Recipient};
use nusb::{Endpoint, Interface, MaybeFuture};

pub struct UsbDevice {
    interface: Interface,
    out_ep_handle: Mutex<Endpoint<Bulk, Out>>,
    in_ep_handle: Mutex<Endpoint<Bulk, In>>,
    pub out_ep: u8,
    pub in_ep: u8,
}

impl UsbDevice {
    /// Discovers and opens a USB device matching the given Vendor ID and Product ID.
    pub fn open(vid: u16, pid: u16, timeout: Duration) -> Result<Self, anyhow::Error> {
        let start = Instant::now();
        loop {
            let found = nusb::list_devices()
                .wait()
                .map_err(|e| anyhow::anyhow!("failed to enumerate USB devices: {e}"))?
                .find(|d| d.vendor_id() == vid && d.product_id() == pid);

            if let Some(info) = found {
                match Self::open_matched(&info) {
                    Ok(dev) => return Ok(dev),
                    Err(e) if start.elapsed() < timeout => {
                        // Device was seen enumerating but not yet claimable (e.g. still
                        // settling after a MaskROM/BL2 handoff reset) - keep polling
                        // rather than surfacing a spurious transient failure.
                        let _ = e;
                    }
                    Err(e) => return Err(e),
                }
            }

            if start.elapsed() >= timeout {
                anyhow::bail!(
                    "Timed out waiting for Amlogic USB device (VID: {:04x}, PID: {:04x})",
                    vid,
                    pid
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn open_matched(info: &nusb::DeviceInfo) -> Result<Self, anyhow::Error> {
        let device = info
            .open()
            .wait()
            .map_err(|e| anyhow::anyhow!("failed to open device: {e}"))?;

        // macOS only auto-sets a configuration for composite/known-class
        // devices; Amlogic's MaskROM/TPL USB stack is vendor class, so it
        // needs an explicit set_configuration there (harmless no-op on
        // platforms where it's already configured).
        if device.active_configuration().is_err() {
            device
                .set_configuration(1)
                .wait()
                .map_err(|e| anyhow::anyhow!("failed to set configuration: {e}"))?;
        }

        let interface = device
            .claim_interface(0)
            .wait()
            .map_err(|e| anyhow::anyhow!("failed to claim interface 0: {e}"))?;

        let (out_ep, in_ep) = probe_bulk_endpoints(&interface).unwrap_or((0x01, 0x81));
        let out_ep_handle = interface
            .endpoint::<Bulk, Out>(out_ep)
            .map_err(|e| anyhow::anyhow!("failed to open bulk OUT endpoint {out_ep:#04x}: {e}"))?;
        let in_ep_handle = interface
            .endpoint::<Bulk, In>(in_ep)
            .map_err(|e| anyhow::anyhow!("failed to open bulk IN endpoint {in_ep:#04x}: {e}"))?;

        Ok(Self {
            interface,
            out_ep_handle: Mutex::new(out_ep_handle),
            in_ep_handle: Mutex::new(in_ep_handle),
            out_ep,
            in_ep,
        })
    }

    /// Performs a USB control transfer. `request_type` follows the standard
    /// USB bit layout (bit 7 set = device-to-host); this device only ever
    /// uses vendor/device-recipient requests.
    pub fn control_transfer(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &mut [u8],
        timeout_ms: u32,
    ) -> Result<usize, anyhow::Error> {
        let timeout = Duration::from_millis(timeout_ms as u64);
        if request_type & 0x80 != 0 {
            let resp = self
                .interface
                .control_in(
                    ControlIn {
                        control_type: ControlType::Vendor,
                        recipient: Recipient::Device,
                        request,
                        value,
                        index,
                        length: data.len() as u16,
                    },
                    timeout,
                )
                .wait()
                .map_err(|e| anyhow::anyhow!("control IN transfer failed: {e}"))?;
            let n = resp.len().min(data.len());
            data[..n].copy_from_slice(&resp[..n]);
            Ok(n)
        } else {
            let len = data.len();
            self.interface
                .control_out(
                    ControlOut {
                        control_type: ControlType::Vendor,
                        recipient: Recipient::Device,
                        request,
                        value,
                        index,
                        data,
                    },
                    timeout,
                )
                .wait()
                .map_err(|e| anyhow::anyhow!("control OUT transfer failed: {e}"))?;
            Ok(len)
        }
    }

    /// Performs a USB bulk write to the given endpoint.
    pub fn bulk_write(
        &self,
        _ep: u8,
        data: &[u8],
        timeout_ms: u32,
    ) -> Result<usize, anyhow::Error> {
        let len = data.len();
        let mut ep = self.out_ep_handle.lock();
        ep.transfer_blocking(
            data.to_vec().into(),
            Duration::from_millis(timeout_ms as u64),
        )
        .into_result()
        .map_err(|e| anyhow::anyhow!("bulk write failed: {e}"))?;
        Ok(len)
    }

    /// Performs a USB bulk read from the given endpoint.
    pub fn bulk_read(
        &self,
        _ep: u8,
        buf: &mut [u8],
        timeout_ms: u32,
    ) -> Result<usize, anyhow::Error> {
        let mut ep = self.in_ep_handle.lock();
        let max_packet = ep.max_packet_size().max(1);
        // IN transfers require requested_len to be a nonzero multiple of the
        // endpoint's max packet size (nusb enforces this) - round the
        // caller's buffer up, then copy back only what was actually read.
        let requested = buf.len().max(max_packet).div_ceil(max_packet) * max_packet;
        let completion = ep.transfer_blocking(
            Buffer::new(requested),
            Duration::from_millis(timeout_ms as u64),
        );
        let received = completion
            .into_result()
            .map_err(|e| anyhow::anyhow!("bulk read failed: {e}"))?;
        let n = received.len().min(buf.len());
        buf[..n].copy_from_slice(&received[..n]);
        Ok(n)
    }

    /// Performs a USB port reset.
    pub fn reset(&self) -> Result<(), anyhow::Error> {
        // nusb's Device::reset isn't reachable from Interface without a
        // clone of the Device; the AMLogic protocol never actually needs a
        // host-initiated port reset in the flows this tool implements
        // (MaskROM/TPL boot handoffs happen device-side), so this is
        // intentionally unimplemented rather than faked.
        anyhow::bail!("USB port reset is not supported by this tool")
    }
}

/// Finds the interface's bulk IN/OUT endpoint addresses from its (already
/// OS-parsed) descriptors.
fn probe_bulk_endpoints(interface: &Interface) -> Option<(u8, u8)> {
    let desc = interface.descriptor()?;
    let mut out_ep = None;
    let mut in_ep = None;
    for ep in desc.endpoints() {
        if ep.transfer_type() != nusb::descriptors::TransferType::Bulk {
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
