#![feature(alloc_io)]

mod adbkey;
mod arm64_image;
mod avb;
mod bootimg;
mod bootimg_bindgen;
mod elf;
mod mkbootimg;
mod usb;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use adbkey::AdbKey;
use android_boot_protocol::adb::AdbClient;
use android_boot_protocol::fastboot::FastbootClient;
use clap::{Args, Parser, Subcommand};
use rootcause::{Result, report};
use usb::UsbTransport;

#[derive(Parser, Debug)]
#[command(
    name = "android-boot",
    about = "Fastboot & ADB client tool for booting and rebooting Android devices"
)]
struct Cli {
    /// USB device serial number filter
    #[arg(short = 's', long = "serial", global = true)]
    serial: Option<String>,

    /// Timeout in seconds to wait for USB device enumeration
    #[arg(short = 't', long = "timeout", default_value = "10", global = true)]
    timeout: u64,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Fastboot protocol commands
    Fastboot(FastbootArgs),
    /// ADB protocol commands
    Adb(AdbArgs),
}

#[derive(Args, Debug)]
struct FastbootArgs {
    #[command(subcommand)]
    subcommand: FastbootSubcommand,
}

#[derive(Subcommand, Debug)]
enum FastbootSubcommand {
    /// Download and boot a kernel/boot image in RAM (`fastboot boot <IMAGE>`)
    Boot {
        /// Path to the boot/kernel image. May be a pre-built Android boot
        /// image (`ANDROID!` magic), or a raw kernel (ELF or flat binary),
        /// in which case it's automatically wrapped in a boot image header
        /// before being sent to the device.
        image: PathBuf,

        /// Path to an initrd/ramdisk image to include (ignored if `image`
        /// is already a pre-built boot image).
        #[arg(short = 'i', long = "initrd")]
        initrd: Option<PathBuf>,

        /// Kernel command line (ignored if `image` is already a pre-built
        /// boot image).
        #[arg(short = 'c', long = "cmdline", default_value = "")]
        cmdline: String,

        /// Physical load address the bootloader should place the kernel
        /// image at (boot image header's `kernel_addr`), as hex (with or
        /// without a `0x` prefix). For an ELF `image` this is normally
        /// auto-derived from the lowest `PT_LOAD` segment's `p_paddr` -
        /// only needed to override that, or to supply an address for an
        /// already-flat (non-ELF) raw binary, which has no address
        /// information of its own. Ignored if `image` is already a
        /// pre-built boot image, or if `--header-version 4` is used (see
        /// `--dram-base`).
        #[arg(long = "kernel-addr")]
        kernel_addr: Option<String>,

        /// Boot image header version to build for a raw (non-pre-built)
        /// `image`. Must match what the target device's bootloader and
        /// currently-flashed `vendor_boot` partition already agree on -
        /// inspect a real `boot.img` from the target device's factory
        /// image (`ANDROID!` magic, header_version at byte offset 40) to
        /// find the right value rather than guessing; GKI-era devices
        /// (2020+, most current Pixels including gs201) use 4.
        #[arg(long = "header-version", default_value = "2")]
        header_version: u32,

        /// Physical base address of usable DRAM on the target device
        /// (e.g. `0x80000000` for Pixel 7a / gs201 - see `board.rs`'s
        /// `DRAM_BASE`). Required (and only used) with `--header-version
        /// 4` and an ELF `image`: header v3/v4 boot images have no
        /// `kernel_addr` field at all, so the kernel's kernel_addr can
        /// only be conveyed by wrapping it in an "arm64 Image" header
        /// (see `arm64_image.rs`) whose `text_offset` is computed
        /// relative to this.
        #[arg(long = "dram-base")]
        dram_base: Option<String>,
    },
}

#[derive(Args, Debug)]
struct AdbArgs {
    #[command(subcommand)]
    subcommand: AdbSubcommand,
}

#[derive(Subcommand, Debug)]
enum AdbSubcommand {
    /// Reboot the device (e.g. `adb reboot bootloader`)
    Reboot {
        /// Target mode to reboot into (e.g. "bootloader")
        #[arg(default_value = "bootloader")]
        target: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let timeout = Duration::from_secs(cli.timeout);
    let serial = cli.serial.as_deref();

    match cli.command {
        Command::Fastboot(fb) => match fb.subcommand {
            FastbootSubcommand::Boot {
                image,
                initrd,
                cmdline,
                kernel_addr,
                header_version,
                dram_base,
            } => handle_fastboot_boot(
                serial,
                timeout,
                &image,
                initrd.as_deref(),
                &cmdline,
                kernel_addr.as_deref(),
                header_version,
                dram_base.as_deref(),
            ),
        },
        Command::Adb(adb) => match adb.subcommand {
            AdbSubcommand::Reboot { target } => handle_adb_reboot(serial, timeout, &target),
        },
    }
}

fn parse_addr(s: &str) -> Result<u32> {
    let trimmed = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(trimmed, 16)
        .map_err(|e| report!("invalid --kernel-addr '{s}' (expected hex, e.g. 0x81000000): {e}"))
}

fn handle_fastboot_boot(
    serial: Option<&str>,
    timeout: Duration,
    image_path: &PathBuf,
    initrd_path: Option<&std::path::Path>,
    cmdline: &str,
    kernel_addr_override: Option<&str>,
    header_version: u32,
    dram_base: Option<&str>,
) -> Result<()> {
    // Build the image before touching USB at all: fail fast on a bad/
    // missing/malformed image file instead of making the caller wait
    // through "Waiting for Fastboot device..." first.
    let raw_image = fs::read(image_path)
        .map_err(|e| report!("failed to read image file '{}': {e}", image_path.display()))?;

    let kernel_addr_override = kernel_addr_override.map(parse_addr).transpose()?;
    let dram_base = dram_base.map(parse_addr).transpose()?;

    let image_data = if mkbootimg::is_boot_image(&raw_image) {
        println!(
            "'{}' is already a boot image, using as-is.",
            image_path.display()
        );
        raw_image
    } else {
        let ramdisk = match initrd_path {
            Some(path) => Some(
                fs::read(path)
                    .map_err(|e| report!("failed to read initrd file '{}': {e}", path.display()))?,
            ),
            None => None,
        };

        // `rust_binary`/Bazel `_raw` targets are full ELF executables,
        // not flat binaries - a bootloader like ABL/LK never parses ELF,
        // it blasts raw bytes into RAM at `kernel_addr` and jumps to the
        // very first byte. Handing an ELF over unmodified means the
        // bootloader executes `\x7fELF...` header bytes as instructions:
        // an immediate crash indistinguishable, from the USB/fastboot
        // side, from a normal boot that then watchdog-resets straight
        // back to the bootloader (see elf.rs's module comment). Flatten
        // it; header v2 conveys the resulting load address via
        // `kernel_addr`, header v4 (GKI, no such field) via an embedded
        // "arm64 Image" header instead (see arm64_image.rs).
        let flat = if elf::is_elf(&raw_image) {
            let flat = elf::flatten_elf(&raw_image).map_err(|e| {
                report!(
                    "failed to flatten ELF image '{}': {e}",
                    image_path.display()
                )
            })?;
            if flat.entry != flat.load_addr {
                println!(
                    "warning: ELF entry point {:#x} != lowest PT_LOAD address {:#x} - _start won't run first; check the linker script",
                    flat.entry, flat.load_addr
                );
            }
            println!(
                "'{}' is an ELF image (entry {:#x}), flattening PT_LOAD segments into a {}-byte raw image based at {:#x}{}...",
                image_path.display(),
                flat.entry,
                flat.bytes.len(),
                flat.load_addr,
                match &ramdisk {
                    Some(r) => format!(", with {}-byte initrd", r.len()),
                    None => String::new(),
                }
            );
            Some(flat)
        } else {
            println!(
                "'{}' is a raw kernel image ({} bytes{}), wrapping in a boot image header...",
                image_path.display(),
                raw_image.len(),
                match &ramdisk {
                    Some(r) => format!(", with {}-byte initrd", r.len()),
                    None => String::new(),
                }
            );
            None
        };

        let image_bytes = if let Some(flat) = &flat {
            flat.bytes.clone()
        } else {
            raw_image.clone()
        };

        if header_version == 4 {
            let flat = flat.as_ref().ok_or_else(|| {
                report!(
                    "--header-version 4 requires an ELF `image` (its PT_LOAD segments carry the \
                     load address needed to build the arm64 Image header, unless the image already \
                     has one baked in); got an already-flat raw binary with no address information \
                     of its own"
                )
            })?;

            let wrapped;
            let kernel_for_v4: &[u8] = if arm64_image::has_arm64_magic(&flat.bytes) {
                // Already self-describing (e.g. boot/src/asm.rs's `_start`
                // bakes one in with the linker's own accurate
                // `__image_size`) - use as-is rather than wrapping again.
                println!(
                    "kernel image already has a valid arm64 Image header baked in, using as-is for header v4..."
                );
                &flat.bytes
            } else {
                let dram_base = dram_base.ok_or_else(|| {
                    report!(
                        "--header-version 4 requires --dram-base (see board.rs's DRAM_BASE for this board) \
                         since this image has no arm64 Image header of its own to derive placement from"
                    )
                })?;
                wrapped =
                    arm64_image::wrap_arm64_image(&flat.bytes, flat.load_addr, dram_base as u64);
                println!(
                    "wrapped kernel in an arm64 Image header (dram_base {:#x}, text_offset {:#x}) for header v4...",
                    dram_base,
                    flat.load_addr - dram_base as u64 - 64
                );
                &wrapped
            };

            let v4_image =
                mkbootimg::build_boot_image_v4(kernel_for_v4, ramdisk.as_deref(), cmdline, 0);
            // ABL appears to require an AVB footer/vbmeta structure to
            // even be *present* to parse the image as valid at all -
            // confirmed against real hardware: a from-scratch image with
            // none bounces straight back to the fastboot menu in ~1-3s
            // regardless of payload, while a byte-corrupted (hash-
            // mismatched, so definitely-failing-verification) copy of
            // the stock signed boot.img - which *does* have this
            // structure - still gets past that gate and attempts to
            // boot. See avb.rs's module comment.
            println!("appending an unsigned AVB footer + vbmeta (algorithm NONE) - see avb.rs...");
            avb::append_footer(&v4_image)
        } else {
            let kernel_addr = match kernel_addr_override {
                Some(addr) => addr,
                None => match &flat {
                    Some(flat) => flat.load_addr as u32,
                    None => {
                        println!(
                            "warning: no --kernel-addr given for a non-ELF raw image; falling back to mkbootimg.py's generic AOSP default, which is almost certainly wrong for this board"
                        );
                        mkbootimg::BootImageParams::default().kernel_addr
                    }
                },
            };
            let params = mkbootimg::BootImageParams {
                cmdline: cmdline.to_string(),
                kernel_addr,
                ..Default::default()
            };
            mkbootimg::build_boot_image(&image_bytes, ramdisk.as_deref(), &params)
        }
    };

    println!("Waiting for Fastboot device...");
    let mut transport = UsbTransport::open_fastboot(serial, timeout)?;
    println!("Fastboot device connected.");

    let client = FastbootClient::new();

    // Query variable if available for diagnostics
    if let Ok(version) = client.getvar(&mut transport, "version") {
        println!("Bootloader version: {version}");
    }

    println!(
        "Downloading image '{}' ({} bytes)...",
        image_path.display(),
        image_data.len()
    );

    client
        .download(&mut transport, &image_data)
        .map_err(|e| report!("fastboot download failed: {e}"))?;

    println!("Image downloaded successfully. Booting...");

    client
        .boot(&mut transport)
        .map_err(|e| report!("fastboot boot failed: {e}"))?;

    println!("Boot command issued successfully.");
    Ok(())
}

fn handle_adb_reboot(serial: Option<&str>, timeout: Duration, target: &str) -> Result<()> {
    let key_path = adbkey::default_key_path();
    let key = AdbKey::load_or_generate(&key_path).map_err(|e| {
        report!(
            "failed to load/generate ADB key at '{}': {e}",
            key_path.display()
        )
    })?;

    println!("Waiting for ADB device...");
    let mut transport = UsbTransport::open_adb(serial, timeout)?;
    println!("ADB device connected.");

    let client = AdbClient::new();
    let dev_info = client
        .connect_with_auth(&mut transport, "host::features=cmd\0", &key)
        .map_err(|e| match e {
            android_boot_protocol::Error::AdbAuthRequired => report!(
                "ADB authentication required: device did not accept our key. \
                 If this is the first time connecting, check the device screen \
                 for an \"Allow USB debugging?\" prompt and accept it, then retry."
            ),
            other => report!("ADB connection handshake failed: {other}"),
        })?;

    println!(
        "Connected to ADB device (protocol version {:#x}, banner: {})",
        dev_info.version, dev_info.banner
    );

    println!("Sending reboot '{target}' command...");
    client
        .reboot(&mut transport, target)
        .map_err(|e| report!("ADB reboot command failed: {e}"))?;

    println!("Reboot command sent successfully. Device is rebooting.");
    Ok(())
}
