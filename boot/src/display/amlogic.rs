//! Superbird (Spotify Car Thing, Amlogic Meson G12A) display hardware:
//! adopts U-Boot's already-programmed VIU OSD1 scanout by reading its
//! canvas-LUT entry rather than reprogramming the VPU from scratch.

use super::{FramebufferInfo, PixelFormat, read32, write32};

/// Amlogic G12A VPU/VCBUS register block base (g12-common.dtsi `vpu@ff900000`).
pub const VPU_BASE: usize = 0xff90_0000;
/// Amlogic G12A DMC/canvas block base (g12-common.dtsi `canvas: video-lut@48` @ 0xff638048).
pub const CANVAS_BASE: usize = 0xff63_8048;

const DMC_CAV_LUT_DATAL: usize = 0x00;
const DMC_CAV_LUT_DATAH: usize = 0x04;
const DMC_CAV_LUT_ADDR: usize = 0x08;
const CANVAS_LUT_RD_EN: u32 = 1 << 8;

const VIU_OSD1_CTRL_STAT: usize = 0x1a10;
const VIU_OSD1_BLK0_CFG_W0: usize = 0x1a1b;
const VIU_OSD1_BLK0_CFG_W1: usize = 0x1a1c;
const VIU_OSD1_BLK0_CFG_W2: usize = 0x1a1d;

const OSD_ENABLE: u32 = 1 << 21;
const OSD_CANVAS_SEL: u32 = 16;

fn read_hardware_canvas(vpu_base: usize, canvas_base: usize) -> Option<FramebufferInfo> {
    unsafe {
        let stat = read32(vpu_base + VIU_OSD1_CTRL_STAT * 4);
        let w0 = read32(vpu_base + VIU_OSD1_BLK0_CFG_W0 * 4);
        let w1 = read32(vpu_base + VIU_OSD1_BLK0_CFG_W1 * 4);
        let w2 = read32(vpu_base + VIU_OSD1_BLK0_CFG_W2 * 4);

        let canvas_idx = (w0 >> OSD_CANVAS_SEL) & 0xff;
        let fmt = match (w0 >> 8) & 0xf {
            0x4 => PixelFormat::Rgb565,
            0x5 => PixelFormat::Xrgb8888,
            0x7 => PixelFormat::Argb8888,
            other => PixelFormat::Unknown(other),
        };

        let width = (((w1 >> 16) & 0x1fff) + 1) as usize;
        let height = (((w2 >> 16) & 0x1fff) + 1) as usize;
        let enabled = (stat & OSD_ENABLE) != 0;

        if !enabled || width == 0 || height == 0 {
            return None;
        }

        write32(
            canvas_base + DMC_CAV_LUT_ADDR,
            CANVAS_LUT_RD_EN | canvas_idx,
        );
        let datal = read32(canvas_base + DMC_CAV_LUT_DATAL);
        let datah = read32(canvas_base + DMC_CAV_LUT_DATAH);

        let addr = ((datal & 0x1fff_ffff) as usize) << 3;
        let stride8 = (((datal >> 29) & 0x7) | ((datah & 0x1ff) << 3)) as usize;
        let stride = stride8 * 8;

        if addr == 0 || stride == 0 {
            return None;
        }

        Some(FramebufferInfo {
            addr,
            stride,
            width,
            height,
            format: fmt,
            active: true,
        })
    }
}

pub fn init_amlogic_vpu(vpu_base: usize, canvas_base: usize) -> bool {
    read_hardware_canvas(vpu_base, canvas_base).is_some()
}

/// Called by `display::init_early`/`display::get_info` to find the
/// currently-programmed scanout buffer. Unlike kernel-lib's `init_early`
/// (which this was ported from), this image has no QEMU-virt build config
/// to distinguish from - see `boot.rs`'s `hvMain` comment - so the panel
/// is always adopted here.
pub(super) fn read_hardware_fb() -> Option<FramebufferInfo> {
    read_hardware_canvas(VPU_BASE, CANVAS_BASE)
}

pub(super) fn kick_scanout() {}
