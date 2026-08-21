//! Pixel 7a / gs201 (Tensor G2) display hardware: adopts ABL's
//! already-programmed DPU_DMA (RDMA) plane instead of reprogramming
//! Samsung's Exynos DECON/DPU pipeline from scratch.
//!
//! gs201's in-tree CAL is `cal_9845` + `cal_9855` (`Kbuild` for
//! `CONFIG_SOC_GS201`). `cal_9865` is Zuma/Tensor G3 and has a
//! *different* RDMA layout (separate WIDTH/HEIGHT registers). Using
//! those offsets here read SRC_OFFSET as "height" and CHROM stride as
//! pixel stride, so adoption always failed and the Google logo stayed.
//!
//! Two other ABL handoff traps, both observed on Tensor/Exynos phones
//! (Pixel 6 simplefb series, uniLoader):
//! 1. The splash plane is often AFBC-compressed. Writing linear pixels
//!    into that buffer is invisible (and skipped entirely by an early
//!    `return None`). We drop AFBC/SBWC and retarget RDMA at a linear
//!    scratch buffer in DRAM.
//! 2. ABL masks DECON's hardware TE trigger (`HW_TRIG_MASK_DECON`)
//!    before jumping to the Image, so even a correct linear write never
//!    reaches the command-mode OLED GRAM. `kick_scanout` unmasks and
//!    software-triggers.

use core::sync::atomic::{AtomicUsize, Ordering};

use super::{FramebufferInfo, PixelFormat, read32, write32};

pub const DPU_L0_DMA_BASE: usize = 0x1C0B_0000;

/// L0–L5 DMA blocks from `gs201-drm-dpu.dtsi` (`drmdpp@1C0B0n000`).
pub const DPP_DMA_BASES: &[usize] = &[
    0x1C0B_0000,
    0x1C0B_1000,
    0x1C0B_2000,
    0x1C0B_3000,
    0x1C0B_4000,
    0x1C0B_5000,
];

/// Scratch linear framebuffer, well above this image (0x8100_0000) and
/// the unused guest slot. 1080×2400×4 ≈ 10 MiB.
const LINEAR_FB: usize = 0x8600_0000;

/// DECON0 `main` block (`drmdecon@1C240000`).
const DECON0_MAIN: usize = 0x1C24_0000;
const GLOBAL_CON: usize = 0x0020;
const TRIG_CON: usize = 0x0030;
const SHD_REG_UP_REQ: usize = 0x0050;
const GLOBAL_CON_DECON_EN: u32 = 1 << 1;
const GLOBAL_CON_DECON_EN_F: u32 = 1 << 0;
const SW_TRIG_EN: u32 = 1 << 8;
const SW_TRIG_DET_EN: u32 = 1 << 1;
const HW_TRIG_EN: u32 = 1 << 0;
const HW_TRIG_MASK_DECON: u32 = 1 << 4;
const SHD_REG_UP_REQ_GLOBAL: u32 = 1 << 31;
const SHD_REG_UP_REQ_FOR_DECON: u32 = 0x3F;

const DMA_SHD_OFFSET: usize = 0x0400;

// cal_9845/regs-dpp.h
const RDMA_ENABLE: usize = 0x0000;
const IDMA_SFR_UPDATE_FORCE: u32 = 1 << 4;
const RDMA_IN_CTRL_0: usize = 0x0008;
const RDMA_SRC_SIZE: usize = 0x0010;
const RDMA_IMG_SIZE: usize = 0x0018;
const RDMA_BASEADDR_Y8: usize = 0x0040;
const RDMA_SRC_STRIDE_0: usize = 0x0050;
const RDMA_SRC_STRIDE_1: usize = 0x0054;
const IDMA_STRIDE_0_SEL: u32 = 1 << 20;

const IDMA_IMG_FORMAT_MASK: u32 = 0x3F << 8;
const IDMA_IMG_FORMAT_SHIFT: u32 = 8;
const IDMA_IMG_FORMAT_XRGB8888: u32 = 7;
const IDMA_AFBC_EN: u32 = 1 << 1;
const IDMA_SBWC_EN: u32 = 1 << 2;
const IDMA_STRIDE_0_MASK: u32 = 0xFFFF;

static ADOPTED_DMA: AtomicUsize = AtomicUsize::new(0);

fn decode_format(raw: u32) -> PixelFormat {
    match (raw & IDMA_IMG_FORMAT_MASK) >> IDMA_IMG_FORMAT_SHIFT {
        9 | 8 => PixelFormat::Rgb565,
        7 | 6 | 5 | 4 => PixelFormat::Xrgb8888,
        3 | 2 | 1 | 0 => PixelFormat::Argb8888,
        other => PixelFormat::Unknown(other),
    }
}

fn bytes_per_pixel(format: PixelFormat) -> usize {
    match format {
        PixelFormat::Rgb565 => 2,
        PixelFormat::Xrgb8888 | PixelFormat::Argb8888 | PixelFormat::Unknown(_) => 4,
    }
}

fn phys_addr(mut addr: usize) -> usize {
    if addr != 0 && addr < 0x8000_0000 {
        addr += 0x8000_0000;
    }
    addr
}

fn read_rdma_at(dma_base: usize, shd: usize) -> Option<(FramebufferInfo, bool)> {
    unsafe {
        let ctrl0 = read32(dma_base + RDMA_IN_CTRL_0 + shd);
        let compressed = ctrl0 & (IDMA_AFBC_EN | IDMA_SBWC_EN) != 0;
        let format = decode_format(ctrl0);

        let src = read32(dma_base + RDMA_SRC_SIZE + shd);
        let img = read32(dma_base + RDMA_IMG_SIZE + shd);
        let mut width = (img & 0x3FFF) as usize;
        let mut height = ((img >> 16) & 0x3FFF) as usize;
        if width == 0 || height == 0 {
            width = (src & 0xFFFF) as usize;
            height = ((src >> 16) & 0xFFFF) as usize;
        }
        if width < 16 || height < 16 || width > 4096 || height > 4096 {
            return None;
        }

        let addr = phys_addr(read32(dma_base + RDMA_BASEADDR_Y8 + shd) as usize);
        if addr == 0 {
            return None;
        }

        let stride_reg = read32(dma_base + RDMA_SRC_STRIDE_1 + shd) & IDMA_STRIDE_0_MASK;
        let stride = if !compressed && stride_reg != 0 {
            stride_reg as usize
        } else {
            width
                * bytes_per_pixel(if compressed {
                    PixelFormat::Xrgb8888
                } else {
                    format
                })
        };

        Some((
            FramebufferInfo {
                addr,
                stride,
                width,
                height,
                format: if compressed {
                    PixelFormat::Xrgb8888
                } else {
                    format
                },
                active: true,
            },
            compressed,
        ))
    }
}

fn read_hardware_rdma(dma_base: usize) -> Option<(FramebufferInfo, bool)> {
    read_rdma_at(dma_base, 0).or_else(|| read_rdma_at(dma_base, DMA_SHD_OFFSET))
}

/// Switch an AFBC/SBWC plane to linear XRGB8888 scanning `LINEAR_FB`.
fn force_linear(dma_base: usize, fb: &FramebufferInfo) {
    unsafe {
        let mut ctrl0 = read32(dma_base + RDMA_IN_CTRL_0);
        ctrl0 &= !(IDMA_AFBC_EN | IDMA_SBWC_EN | IDMA_IMG_FORMAT_MASK);
        ctrl0 |= IDMA_IMG_FORMAT_XRGB8888 << IDMA_IMG_FORMAT_SHIFT;
        write32(dma_base + RDMA_IN_CTRL_0, ctrl0);

        write32(dma_base + RDMA_BASEADDR_Y8, LINEAR_FB as u32);
        write32(
            dma_base + RDMA_SRC_STRIDE_0,
            read32(dma_base + RDMA_SRC_STRIDE_0) | IDMA_STRIDE_0_SEL,
        );
        write32(dma_base + RDMA_SRC_STRIDE_1, fb.stride as u32);

        let en = read32(dma_base + RDMA_ENABLE);
        write32(dma_base + RDMA_ENABLE, en | IDMA_SFR_UPDATE_FORCE);
    }
}

pub(super) fn read_hardware_fb() -> Option<FramebufferInfo> {
    for &base in DPP_DMA_BASES {
        if let Some((mut fb, compressed)) = read_hardware_rdma(base) {
            if compressed {
                fb.addr = LINEAR_FB;
                fb.stride = fb.width * 4;
                fb.format = PixelFormat::Xrgb8888;
                force_linear(base, &fb);
            }
            ADOPTED_DMA.store(base, Ordering::Release);
            return Some(fb);
        }
    }
    None
}

/// Command-mode DSI panels (Pixel OLEDs) keep the last frame in GRAM.
/// ABL masks the TE trigger before Image entry, so DRAM writes are not
/// visible until DECON is unmasked and software-triggered.
pub(super) fn kick_scanout() {
    unsafe {
        let mut g = read32(DECON0_MAIN + GLOBAL_CON);
        if g & GLOBAL_CON_DECON_EN == 0 {
            g |= GLOBAL_CON_DECON_EN | GLOBAL_CON_DECON_EN_F;
            write32(DECON0_MAIN + GLOBAL_CON, g);
        }

        let dma = ADOPTED_DMA.load(Ordering::Acquire);
        if dma != 0 {
            let en = read32(dma + RDMA_ENABLE);
            write32(dma + RDMA_ENABLE, en | IDMA_SFR_UPDATE_FORCE);
        }

        write32(
            DECON0_MAIN + SHD_REG_UP_REQ,
            SHD_REG_UP_REQ_GLOBAL | SHD_REG_UP_REQ_FOR_DECON,
        );

        // Drop HW_TRIG_MASK_DECON (what ABL sets to freeze the splash),
        // keep/restore HW_TRIG_EN, and pulse a software trigger.
        let t = read32(DECON0_MAIN + TRIG_CON);
        write32(
            DECON0_MAIN + TRIG_CON,
            (t & !HW_TRIG_MASK_DECON) | HW_TRIG_EN | SW_TRIG_EN | SW_TRIG_DET_EN,
        );
    }
}
