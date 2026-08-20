#!/usr/bin/env python3
"""Build a minimal Apple DeviceTree binary for vmapple XNU and test parsing."""

import struct

PROP_NAME_LEN = 32


class Node:
    def __init__(self, name: str):
        self.name = name
        self.properties: list[tuple[str, bytes]] = []
        self.children: list[Node] = []

    def add_prop(self, name: str, val: bytes):
        self.properties.append((name, val))

    def add_str(self, name: str, s: str):
        self.add_prop(name, s.encode("utf-8") + b"\x00")

    def add_u32(self, name: str, val: int):
        self.add_prop(name, struct.pack("<I", val))

    def add_u64(self, name: str, val: int):
        self.add_prop(name, struct.pack("<Q", val))

    def add_child(self, child: "Node"):
        self.children.append(child)

    def serialize(self) -> bytes:
        out = bytearray()
        # Header: nProperties (u32), nChildren (u32)
        # Ensure 'name' property is first if present
        props = [("name", self.name.encode("utf-8") + b"\x00")] + self.properties
        out += struct.pack("<II", len(props), len(self.children))
        for pname, pval in props:
            # 32 bytes null-padded name
            name_bytes = pname.encode("utf-8")[:PROP_NAME_LEN]
            name_bytes = name_bytes.ljust(PROP_NAME_LEN, b"\x00")
            out += name_bytes
            # length (u32, with optional placeholder flag in bit 31)
            plen = len(pval)
            out += struct.pack("<I", plen)
            # value padded to 4 bytes
            aligned = (plen + 3) & ~3
            out += pval.ljust(aligned, b"\x00")
        for child in self.children:
            out += child.serialize()
        return bytes(out)


def build_vmapple_dtb() -> bytes:
    root = Node("device-tree")
    root.add_str("model", "Apple Virtual Platform")
    root.add_str("target-type", "VirtualMachine")
    root.add_str("compatible", "apple,vmapple")
    root.add_u32("#address-cells", 2)
    root.add_u32("#size-cells", 2)
    root.add_u32("clock-frequency", 24000000)

    # /chosen
    chosen = Node("chosen")
    chosen.add_u32("debug-enabled", 1)
    chosen.add_str("firmware-version", "OpenDarwin-HV 1.0")
    chosen.add_str("boot-args", "-v serial=3 debug=0x14e")
    root.add_child(chosen)

    # /defaults
    defaults = Node("defaults")
    # Point serial-device to UART phandle (0x100)
    defaults.add_u32("serial-device", 0x100)
    root.add_child(defaults)

    # /cpus
    cpus = Node("cpus")
    cpus.add_u32("#address-cells", 1)
    cpus.add_u32("#size-cells", 0)

    cpu0 = Node("cpu0")
    cpu0.add_str("device_type", "cpu")
    cpu0.add_str("state", "running")
    cpu0.add_u32("timebase-frequency", 24000000)
    cpu0.add_u32("bus-frequency", 100000000)
    cpu0.add_u32("cpu-frequency", 1000000000)
    cpu0.add_u32("memory-frequency", 100000000)
    cpu0.add_u32("peripheral-frequency", 24000000)
    cpu0.add_u32("reg", 0)
    cpus.add_child(cpu0)
    root.add_child(cpus)

    # /arm-io (SoC bus)
    arm_io = Node("arm-io")
    arm_io.add_str("device_type", "arm-io")
    arm_io.add_str("compatible", "arm-io,vmapple")
    arm_io.add_u32("#address-cells", 2)
    arm_io.add_u32("#size-cells", 2)
    # ranges: child_addr (u64), parent_phys (u64), size (u64)
    # Map SoC space starting at 0x0 -> physical 0x0
    arm_io.add_prop("ranges", struct.pack("<3Q", 0, 0, 0x100000000))

    # /arm-io/uart0 (PL011 at QEMU's 0x09000000)
    uart = Node("uart0")
    uart.add_str("device_type", "serial")
    uart.add_str("compatible", "arm,pl011")
    # reg: offset within soc_base (0x09000000), size (0x1000)
    uart.add_prop("reg", struct.pack("<2Q", 0x09000000, 0x1000))
    uart.add_u32("AAPL,phandle", 0x100)
    uart.add_u32("clock-frequency", 24000000)
    uart.add_u32("current-speed", 115200)
    arm_io.add_child(uart)

    root.add_child(arm_io)

    return root.serialize()


if __name__ == "__main__":
    dtb = build_vmapple_dtb()
    print(f"Generated Apple DTB: {len(dtb)} bytes")
    with open("/tmp/test_adt.bin", "wb") as f:
        f.write(dtb)
