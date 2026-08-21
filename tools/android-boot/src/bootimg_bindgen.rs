// Copyright 2024 The Android Open Source Project
// Copyright 2007 The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Hand-written equivalent of the `bootimg_bindgen` crate normally produced
//! by `bindgen` (via `rust_bindgen`/Soong) from AOSP's
//! `system/tools/mkbootimg/include/bootimg/bootimg.h`. Since this repo's
//! Bazel setup doesn't run `bindgen` at build time, these `#[repr(C, packed)]`
//! structs are transcribed field-for-field from that header (tag
//! `android-17.0.0_r1`) instead, matching the layout `bindgen` would emit
//! (including the `_base: <parent>` embedding it uses to flatten C++
//! single inheritance). This lets us use the upstream `bootimg.rs` (see
//! `bootimg.rs` in this directory) unmodified.
#![allow(non_camel_case_types)]
// v0/v1/v2 (used to build boot images) are exercised; v3/v4/vendor_boot
// structs are transcribed for API completeness/future use (e.g. parsing
// pre-built images) but not all constructed yet.
#![allow(dead_code)]

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

pub const BOOT_MAGIC: [u8; 9] = *b"ANDROID!\0";
pub const BOOT_MAGIC_SIZE: u32 = 8;
pub const BOOT_NAME_SIZE: u32 = 16;
pub const BOOT_ARGS_SIZE: u32 = 512;
pub const BOOT_EXTRA_ARGS_SIZE: u32 = 1024;

pub const VENDOR_BOOT_MAGIC: [u8; 9] = *b"VNDRBOOT\0";
pub const VENDOR_BOOT_MAGIC_SIZE: u32 = 8;
pub const VENDOR_BOOT_ARGS_SIZE: u32 = 2048;
pub const VENDOR_BOOT_NAME_SIZE: u32 = 16;

pub const VENDOR_RAMDISK_TYPE_NONE: u32 = 0;
pub const VENDOR_RAMDISK_TYPE_PLATFORM: u32 = 1;
pub const VENDOR_RAMDISK_TYPE_RECOVERY: u32 = 2;
pub const VENDOR_RAMDISK_TYPE_DLKM: u32 = 3;
pub const VENDOR_RAMDISK_NAME_SIZE: u32 = 32;
pub const VENDOR_RAMDISK_TABLE_ENTRY_BOARD_ID_SIZE: u32 = 16;

// `#[derive(Default)]` doesn't work here: `[u8; N]: Default` is only
// implemented by core for `N <= 32`, and this header contains larger
// fixed-size arrays (`cmdline`, `extra_cmdline`). Implemented manually below.
#[repr(C, packed)]
#[derive(FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq, Copy, Clone, Debug)]
pub struct boot_img_hdr_v0 {
    pub magic: [u8; 8],
    pub kernel_size: u32,
    pub kernel_addr: u32,
    pub ramdisk_size: u32,
    pub ramdisk_addr: u32,
    pub second_size: u32,
    pub second_addr: u32,
    pub tags_addr: u32,
    pub page_size: u32,
    pub header_version: u32,
    pub os_version: u32,
    pub name: [u8; 16],
    pub cmdline: [u8; 512],
    pub id: [u32; 8],
    pub extra_cmdline: [u8; 1024],
}

impl Default for boot_img_hdr_v0 {
    fn default() -> Self {
        Self {
            magic: [0; 8],
            kernel_size: 0,
            kernel_addr: 0,
            ramdisk_size: 0,
            ramdisk_addr: 0,
            second_size: 0,
            second_addr: 0,
            tags_addr: 0,
            page_size: 0,
            header_version: 0,
            os_version: 0,
            name: [0; 16],
            cmdline: [0; 512],
            id: [0; 8],
            extra_cmdline: [0; 1024],
        }
    }
}

#[repr(C, packed)]
#[derive(FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq, Copy, Clone, Debug, Default)]
pub struct boot_img_hdr_v1 {
    pub _base: boot_img_hdr_v0,
    pub recovery_dtbo_size: u32,
    pub recovery_dtbo_offset: u64,
    pub header_size: u32,
}

#[repr(C, packed)]
#[derive(FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq, Copy, Clone, Debug, Default)]
pub struct boot_img_hdr_v2 {
    pub _base: boot_img_hdr_v1,
    pub dtb_size: u32,
    pub dtb_addr: u64,
}

#[repr(C, packed)]
#[derive(FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq, Copy, Clone, Debug)]
pub struct boot_img_hdr_v3 {
    pub magic: [u8; 8],
    pub kernel_size: u32,
    pub ramdisk_size: u32,
    pub os_version: u32,
    pub header_size: u32,
    pub reserved: [u32; 4],
    pub header_version: u32,
    pub cmdline: [u8; 1536], // BOOT_ARGS_SIZE + BOOT_EXTRA_ARGS_SIZE
}

impl Default for boot_img_hdr_v3 {
    fn default() -> Self {
        Self {
            magic: [0; 8],
            kernel_size: 0,
            ramdisk_size: 0,
            os_version: 0,
            header_size: 0,
            reserved: [0; 4],
            header_version: 0,
            cmdline: [0; 1536],
        }
    }
}

#[repr(C, packed)]
#[derive(FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq, Copy, Clone, Debug, Default)]
pub struct boot_img_hdr_v4 {
    pub _base: boot_img_hdr_v3,
    pub signature_size: u32,
}

#[repr(C, packed)]
#[derive(FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq, Copy, Clone, Debug)]
pub struct vendor_boot_img_hdr_v3 {
    pub magic: [u8; 8],
    pub header_version: u32,
    pub page_size: u32,
    pub kernel_addr: u32,
    pub ramdisk_addr: u32,
    pub vendor_ramdisk_size: u32,
    pub cmdline: [u8; 2048],
    pub tags_addr: u32,
    pub name: [u8; 16],
    pub header_size: u32,
    pub dtb_size: u32,
    pub dtb_addr: u64,
}

impl Default for vendor_boot_img_hdr_v3 {
    fn default() -> Self {
        Self {
            magic: [0; 8],
            header_version: 0,
            page_size: 0,
            kernel_addr: 0,
            ramdisk_addr: 0,
            vendor_ramdisk_size: 0,
            cmdline: [0; 2048],
            tags_addr: 0,
            name: [0; 16],
            header_size: 0,
            dtb_size: 0,
            dtb_addr: 0,
        }
    }
}

#[repr(C, packed)]
#[derive(FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq, Copy, Clone, Debug, Default)]
pub struct vendor_boot_img_hdr_v4 {
    pub _base: vendor_boot_img_hdr_v3,
    pub vendor_ramdisk_table_size: u32,
    pub vendor_ramdisk_table_entry_num: u32,
    pub vendor_ramdisk_table_entry_size: u32,
    pub bootconfig_size: u32,
}

#[repr(C, packed)]
#[derive(FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq, Copy, Clone, Debug, Default)]
pub struct vendor_ramdisk_table_entry_v4 {
    pub ramdisk_size: u32,
    pub ramdisk_offset: u32,
    pub ramdisk_type: u32,
    pub ramdisk_name: [u8; 32],
    pub board_id: [u32; 16],
}

#[cfg(test)]
mod tests {
    use super::*;

    // Sanity-check struct sizes against the packed C layout in
    // bootimg.h (tag android-17.0.0_r1).
    #[test]
    fn header_sizes_match_upstream() {
        assert_eq!(core::mem::size_of::<boot_img_hdr_v0>(), 1632);
        assert_eq!(core::mem::size_of::<boot_img_hdr_v1>(), 1648);
        assert_eq!(core::mem::size_of::<boot_img_hdr_v2>(), 1660);
        assert_eq!(core::mem::size_of::<boot_img_hdr_v3>(), 1580);
        assert_eq!(core::mem::size_of::<boot_img_hdr_v4>(), 1584);
    }
}
