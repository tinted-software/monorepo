//! Minimal Apple DeviceTree (ADT) binary format serializer for XNU.
//!
//! Implements the flattened DeviceTree layout XNU's `SecureDTInit` /
//! `DTInit` parser expects (see `pexpert/pexpert/device_tree.h`).

use core::mem::size_of;

const PROP_NAME_LEN: usize = 32;

pub struct DtWriter<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> DtWriter<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, offset: 0 }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    fn write_bytes(&mut self, data: &[u8]) {
        let end = self.offset + data.len();
        if end <= self.buf.len() {
            self.buf[self.offset..end].copy_from_slice(data);
        }
        self.offset = end;
    }

    fn write_u32(&mut self, val: u32) {
        self.write_bytes(&val.to_le_bytes());
    }

    fn write_u64(&mut self, val: u64) {
        self.write_bytes(&val.to_le_bytes());
    }

    pub fn write_prop(&mut self, name: &str, val: &[u8]) {
        let mut name_buf = [0u8; PROP_NAME_LEN];
        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(PROP_NAME_LEN);
        name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
        self.write_bytes(&name_buf);

        let len = val.len() as u32;
        self.write_u32(len);

        self.write_bytes(val);
        // Pad value to 4-byte boundary
        let pad = ((len + 3) & !3) as usize - len as usize;
        if pad > 0 {
            let zeros = [0u8; 4];
            self.write_bytes(&zeros[..pad]);
        }
    }

    pub fn write_str_prop(&mut self, name: &str, val: &str) {
        let mut bytes = [0u8; 64];
        let val_bytes = val.as_bytes();
        let len = val_bytes.len().min(63);
        bytes[..len].copy_from_slice(&val_bytes[..len]);
        bytes[len] = 0; // null terminator
        self.write_prop(name, &bytes[..len + 1]);
    }

    pub fn write_u32_prop(&mut self, name: &str, val: u32) {
        self.write_prop(name, &val.to_le_bytes());
    }

    pub fn write_u64_prop(&mut self, name: &str, val: u64) {
        self.write_prop(name, &val.to_le_bytes());
    }
}

/// Generates a complete Apple DeviceTree for vmapple XNU.
///
/// Returns the number of bytes written to `dst`.
pub fn generate_vmapple_adt(
    dst: &mut [u8],
    dram_base: u64,
    dram_size: u64,
    pl011_base: u64,
) -> usize {
    let mut w = DtWriter::new(dst);

    // Root node: "device-tree"
    // 6 properties: name, model, target-type, compatible, #address-cells, #size-cells
    // 4 children: chosen, defaults, cpus, arm-io
    w.write_u32(6); // nProperties
    w.write_u32(4); // nChildren
    w.write_str_prop("name", "device-tree");
    w.write_str_prop("model", "Apple Virtual Platform");
    w.write_str_prop("target-type", "VirtualMachine");
    w.write_str_prop("compatible", "apple,vmapple");
    w.write_u32_prop("#address-cells", 2);
    w.write_u32_prop("#size-cells", 2);

    // /chosen node
    // 7 properties: name, debug-enabled, firmware-version, boot-args, dram-base, dram-size, random-seed
    // 1 child: memory-map
    w.write_u32(7); // nProperties
    w.write_u32(1); // nChildren
    w.write_str_prop("name", "chosen");
    w.write_u32_prop("debug-enabled", 1);
    w.write_str_prop("firmware-version", "OpenDarwin-HV 1.0");
    w.write_str_prop("boot-args", "-v serial=3 debug=0x14e");
    w.write_u64_prop("dram-base", dram_base);
    w.write_u64_prop("dram-size", dram_size);
    let mut seed = [0u8; 256];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = ((i as u8).wrapping_mul(37) ^ 0xa5) | 1; // non-zero entropy bytes
    }
    w.write_prop("random-seed", &seed);

    // /chosen/memory-map node (empty map required by arm_vm_prot_init)
    // 1 property: name
    // 0 children
    w.write_u32(1); // nProperties
    w.write_u32(0); // nChildren
    w.write_str_prop("name", "memory-map");
    // /defaults node
    // 2 properties: name, serial-device
    // 0 children
    w.write_u32(2); // nProperties
    w.write_u32(0); // nChildren
    w.write_str_prop("name", "defaults");
    w.write_u32_prop("serial-device", 0x100); // phandle 0x100 -> uart0

    // /cpus node
    // 3 properties: name, #address-cells, #size-cells
    // 1 child: cpu0
    w.write_u32(3); // nProperties
    w.write_u32(1); // nChildren
    w.write_str_prop("name", "cpus");
    w.write_u32_prop("#address-cells", 1);
    w.write_u32_prop("#size-cells", 0);

    // /cpus/cpu0 node
    // 9 properties: name, device_type, state, reg, timebase-frequency, bus-frequency,
    //               cpu-frequency, memory-frequency, peripheral-frequency
    // 0 children
    w.write_u32(9); // nProperties
    w.write_u32(0); // nChildren
    w.write_str_prop("name", "cpu0");
    w.write_str_prop("device_type", "cpu");
    w.write_str_prop("state", "running");
    w.write_u32_prop("reg", 0); // Mandatory physical cpu_id
    w.write_u32_prop("timebase-frequency", 24_000_000);
    w.write_u32_prop("bus-frequency", 100_000_000);
    w.write_u32_prop("cpu-frequency", 1_000_000_000);
    w.write_u32_prop("memory-frequency", 100_000_000);
    w.write_u32_prop("peripheral-frequency", 24_000_000);
    // /arm-io node (SoC bus)
    // 6 properties: name, device_type, compatible, #address-cells, #size-cells, ranges
    // 2 children: uart0, gic
    w.write_u32(6); // nProperties
    w.write_u32(2); // nChildren
    w.write_str_prop("name", "arm-io");
    w.write_str_prop("device_type", "arm-io");
    w.write_str_prop("compatible", "arm-io,vmapple");
    w.write_u32_prop("#address-cells", 2);
    w.write_u32_prop("#size-cells", 2);
    // ranges: child_addr (u64), parent_phys (u64), size (u64)
    // Set arm-io SoC base to 0x0800_0000 (covering QEMU GIC and UART MMIO space).
    let soc_base: u64 = 0x0800_0000;
    let ranges: [u64; 3] = [0, soc_base, 0x0800_0000];
    let ranges_bytes =
        unsafe { core::slice::from_raw_parts(ranges.as_ptr() as *const u8, size_of::<[u64; 3]>()) };
    w.write_prop("ranges", ranges_bytes);

    // /arm-io/uart0 (PL011)
    // 6 properties: name, device_type, compatible, reg, AAPL,phandle, clock-frequency
    // 0 children
    w.write_u32(6); // nProperties
    w.write_u32(0); // nChildren
    w.write_str_prop("name", "uart0");
    w.write_str_prop("device_type", "serial");
    w.write_str_prop("compatible", "arm,pl011");
    // reg: offset from arm-io base (u64), size (u64)
    // In QEMU virt: UART is at 0x0900_0000, so offset is 0x0100_0000
    let reg: [u64; 2] = [pl011_base.saturating_sub(soc_base), 0x1000];
    let reg_bytes =
        unsafe { core::slice::from_raw_parts(reg.as_ptr() as *const u8, size_of::<[u64; 2]>()) };
    w.write_prop("reg", reg_bytes);
    w.write_u32_prop("AAPL,phandle", 0x100);
    w.write_u32_prop("clock-frequency", 24_000_000);

    // /arm-io/gic (GICv3 interrupt controller)
    // 4 properties: name, device_type, compatible, reg
    // 0 children
    w.write_u32(4); // nProperties
    w.write_u32(0); // nChildren
    w.write_str_prop("name", "gic");
    w.write_str_prop("device_type", "interrupt-controller");
    w.write_str_prop("compatible", "arm,gic-v3");
    // reg: [gicd_offset, gicd_size, gicr_offset, gicr_size]
    // In QEMU virt: GICD is at 0x0800_0000 (offset 0x0), GICR is at 0x080A_0000 (offset 0x000A_0000)
    let gic_reg: [u64; 4] = [0x0000_0000, 0x0001_0000, 0x000a_0000, 0x0020_0000];
    let gic_reg_bytes = unsafe {
        core::slice::from_raw_parts(gic_reg.as_ptr() as *const u8, size_of::<[u64; 4]>())
    };
    w.write_prop("reg", gic_reg_bytes);

    w.offset()
}
