//! USB Class / Subclass / Protocol identifiers for Android Fastboot and ADB interfaces.

/// Android ADB USB interface class.
pub const ADB_CLASS: u8 = 0xFF;
/// Android ADB USB interface subclass.
pub const ADB_SUBCLASS: u8 = 0x42;
/// Android ADB USB interface protocol.
pub const ADB_PROTOCOL: u8 = 0x01;

/// Android Fastboot USB interface class.
pub const FASTBOOT_CLASS: u8 = 0xFF;
/// Android Fastboot USB interface subclass.
pub const FASTBOOT_SUBCLASS: u8 = 0x42;
/// Android Fastboot USB interface protocol.
pub const FASTBOOT_PROTOCOL: u8 = 0x03;

/// Common Google/AOSP USB Vendor ID.
pub const GOOGLE_USB_VID: u16 = 0x18D1;

/// Returns true if the interface descriptor matches standard Android ADB interface.
pub fn is_adb_interface(class: u8, subclass: u8, protocol: u8) -> bool {
    class == ADB_CLASS && subclass == ADB_SUBCLASS && protocol == ADB_PROTOCOL
}

/// Returns true if the interface descriptor matches standard Android Fastboot interface.
pub fn is_fastboot_interface(class: u8, subclass: u8, protocol: u8) -> bool {
    class == FASTBOOT_CLASS && subclass == FASTBOOT_SUBCLASS && protocol == FASTBOOT_PROTOCOL
}
