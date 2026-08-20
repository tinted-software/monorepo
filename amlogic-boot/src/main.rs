//! Amlogic USB boot host tool for OpenDarwin and Meson G12A.
//!
//! Replaces `scripts/superbird-ramboot.py` and `pyamlboot` with a fast, native Rust tool.

mod protocol;
mod socid;
mod usb;

use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use protocol::AmlogicDevice;
use socid::SocId;

#[derive(Parser)]
#[command(
    name = "amlogic-boot",
    version = "0.1.0",
    about = "Amlogic USB Boot Host Utility"
)]
struct Cli {
    /// Timeout in seconds waiting for USB device to enumerate
    #[arg(short, long, default_value_t = 30)]
    timeout: u64,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Query the SoC ID and current bootloader stage
    Identify,

    /// Bootstrap a G12A device from MaskROM mode (Stage 0.16) using a U-Boot/FIP binary
    BootG12 {
        /// Path to U-Boot binary (FIP / u-boot.bin)
        #[arg(short, long)]
        uboot: PathBuf,
    },
    Ramboot {
        /// Path to raw kernel Image binary
        #[arg(short, long, default_value = "target/superbird/Image")]
        image: PathBuf,

        /// Path to Device Tree Blob (.dtb)
        #[arg(short, long)]
        dtb: Option<PathBuf>,

        /// DRAM address to load kernel image (2 MiB aligned)
        #[arg(long, default_value = "0x02000000", value_parser = parse_hex_or_dec)]
        addr_image: u32,

        /// DRAM address to load DTB
        #[arg(long, default_value = "0x04000000", value_parser = parse_hex_or_dec)]
        addr_dtb: u32,

        /// Boot method (booti or go)
        #[arg(short, long, default_value = "booti")]
        method: String,
    },

    /// Write binary file to DRAM memory
    WriteMem {
        /// Target DRAM address
        #[arg(short, long, value_parser = parse_hex_or_dec)]
        addr: u32,

        /// Input file path
        #[arg(short, long)]
        file: PathBuf,

        /// Chunk transfer block size in bytes
        #[arg(short, long, default_value_t = 512)]
        block_size: usize,
    },

    /// Read memory from DRAM and save to file
    ReadMem {
        /// Source DRAM address
        #[arg(short, long, value_parser = parse_hex_or_dec)]
        addr: u32,

        /// Number of bytes to read
        #[arg(short, long, value_parser = parse_hex_or_dec)]
        len: usize,

        /// Output file path
        #[arg(short, long)]
        out: PathBuf,
    },

    /// Execute an arbitrary U-Boot command over USB (e.g. "fastboot", "reset")
    Cmd {
        /// U-Boot command string
        command: String,
    },

    /// Execute code directly at a DRAM address (REQ_RUN_IN_ADDR) - works at
    /// any stage including raw MaskROM, unlike `ramboot`'s U-Boot text
    /// command handoff. Use for bare-metal images (no U-Boot/DTB needed).
    Run {
        /// DRAM address to jump to
        #[arg(short, long, value_parser = parse_hex_or_dec)]
        addr: u32,

        /// Cut power/USB handoff timeout instead of keeping the SoC
        /// powered on after handoff (clears FLAG_KEEP_POWER_ON, which is
        /// set by default - matches `boot_g12`'s own `run()` call).
        #[arg(long)]
        no_keep_power: bool,
    },
}

fn parse_hex_or_dec(s: &str) -> Result<u32, String> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        u32::from_str_radix(&s[2..], 16).map_err(|e| e.to_string())
    } else {
        s.parse::<u32>().map_err(|e| e.to_string())
    }
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    let timeout = Duration::from_secs(cli.timeout);

    println!("==> Looking for Amlogic SoC in USB Boot Mode (VID: 0x1b8e, PID: 0xc003)...");
    let dev = AmlogicDevice::open(timeout)?;
    println!("==> Connected to device at {}", dev.identify()?);

    match cli.command {
        Commands::Identify => {
            let socid = dev.identify()?;
            println!("\nDevice Identification:");
            println!("  ROM Version:     {}.{}", socid.rom_major, socid.rom_minor);
            println!(
                "  Stage Version:   {}.{} ({})",
                socid.stage_major,
                socid.stage_minor,
                socid.stage_name()
            );
            println!(
                "  Need Password:   {}",
                if socid.need_password { "Yes" } else { "No" }
            );
            println!(
                "  Password Status: {}",
                if socid.password_ok { "OK" } else { "None" }
            );
        }

        Commands::Ramboot {
            image,
            dtb,
            addr_image,
            addr_dtb,
            method,
        } => {
            let socid = dev.identify()?;
            if socid.stage_minor != SocId::STAGE_MINOR_TPL {
                eprintln!(
                    "WARNING: Expected TPL stage (U-Boot USB Burn Mode), got stage {}. Ensure the board is in burn mode.",
                    socid.stage_minor
                );
            }

            // Read Image file
            let mut img_file = File::open(&image).map_err(|e| {
                anyhow::anyhow!("Failed to open image '{}': {}", image.display(), e)
            })?;
            let mut img_data = Vec::new();
            img_file.read_to_end(&mut img_data)?;

            println!(
                "==> Uploading kernel image '{}' ({} bytes) to {:#010x}...",
                image.display(),
                img_data.len(),
                addr_image
            );
            dev.write_large_memory(
                addr_image,
                &img_data,
                512,
                Some(|done, total| {
                    print!(
                        "\r    Progress: {}/{} bytes ({:.1}%)",
                        done,
                        total,
                        (done as f64 / total as f64) * 100.0
                    );
                    let _ = std::io::stdout().flush();
                }),
            )?;
            println!("\n    [Image uploaded successfully]");

            // Read DTB if provided
            let has_dtb = if let Some(ref dtb_path) = dtb {
                let mut dtb_file = File::open(dtb_path).map_err(|e| {
                    anyhow::anyhow!("Failed to open DTB '{}': {}", dtb_path.display(), e)
                })?;
                let mut dtb_data = Vec::new();
                dtb_file.read_to_end(&mut dtb_data)?;

                println!(
                    "==> Uploading DTB '{}' ({} bytes) to {:#010x}...",
                    dtb_path.display(),
                    dtb_data.len(),
                    addr_dtb
                );
                dev.write_large_memory(
                    addr_dtb,
                    &dtb_data,
                    512,
                    Some(|done, total| {
                        print!(
                            "\r    Progress: {}/{} bytes ({:.1}%)",
                            done,
                            total,
                            (done as f64 / total as f64) * 100.0
                        );
                        let _ = std::io::stdout().flush();
                    }),
                )?;
                println!("\n    [DTB uploaded successfully]");
                true
            } else {
                false
            };

            // Execute boot command
            let cmd = if method == "go" || !has_dtb {
                format!("go {:#x}", addr_image)
            } else {
                format!("booti {:#x} - {:#x}", addr_image, addr_dtb)
            };

            println!("==> Booting payload via U-Boot command: '{}'...", cmd);
            match dev.bulk_cmd(&cmd) {
                Ok(Some(resp)) => {
                    println!("    Response: {}", resp.trim());
                }
                Ok(None) | Err(_) => {
                    // booti drops USB interface immediately on handoff, so disconnect/timeout means success
                    println!("    (Device successfully handed off to OpenDarwin kernel)");
                }
            }

            println!("\n==> RAM-boot initiated. Watch display panel for boot beacon output!");
        }

        Commands::WriteMem {
            addr,
            file,
            block_size,
        } => {
            let mut f = File::open(&file)
                .map_err(|e| anyhow::anyhow!("Failed to open file '{}': {}", file.display(), e))?;
            let mut data = Vec::new();
            f.read_to_end(&mut data)?;

            println!("==> Writing {} bytes to {:#010x}...", data.len(), addr);
            dev.write_large_memory(
                addr,
                &data,
                block_size,
                Some(|done, total| {
                    print!(
                        "\r    Progress: {}/{} bytes ({:.1}%)",
                        done,
                        total,
                        (done as f64 / total as f64) * 100.0
                    );
                    let _ = std::io::stdout().flush();
                }),
            )?;
            println!("\n[Done]");
        }

        Commands::ReadMem { addr, len, out } => {
            println!("==> Reading {} bytes from {:#010x}...", len, addr);
            let mut data = Vec::new();
            let mut offset = 0;
            while offset < len {
                let chunk_len = (len - offset).min(64);
                let chunk = dev.read_memory(addr + offset as u32, chunk_len)?;
                data.extend_from_slice(&chunk);
                offset += chunk_len;
            }

            let mut f = File::create(&out).map_err(|e| {
                anyhow::anyhow!("Failed to create output file '{}': {}", out.display(), e)
            })?;
            f.write_all(&data)?;
            println!("==> Wrote {} bytes to '{}'", data.len(), out.display());
        }

        Commands::Cmd { command } => {
            println!("==> Executing U-Boot command: '{}'", command);
            match dev.bulk_cmd(&command)? {
                Some(resp) => println!("Response:\n{}", resp),
                None => println!("(No response / device rebooted)"),
            }
        }

        Commands::BootG12 { uboot } => {
            let mut f = File::open(&uboot).map_err(|e| {
                anyhow::anyhow!("Failed to open u-boot binary '{}': {}", uboot.display(), e)
            })?;
            let mut data = Vec::new();
            f.read_to_end(&mut data)?;

            println!(
                "==> Bootstrapping G12A device from MaskROM mode with '{}' ({} bytes)...",
                uboot.display(),
                data.len()
            );
            dev.boot_g12(&data)?;
            println!("==> Device bootstrapped into U-Boot USB Burn Mode (TPL)!");
        }

        Commands::Run {
            addr,
            no_keep_power,
        } => {
            let keep_power = !no_keep_power;
            println!(
                "==> Executing code at {:#010x} (keep_power={})...",
                addr, keep_power
            );
            dev.run(addr, keep_power)?;
            println!("==> Handed off - device is now running the loaded image.");
        }
    }

    Ok(())
}
