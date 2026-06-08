/// Minimal Mach-O reader/writer used by setsegname and kextsymboltool.
import Foundation

// MARK: - Constants (from mach-o/loader.h & fat.h)

public let MH_MAGIC: UInt32 = 0xFEED_FACE
public let MH_CIGAM: UInt32 = 0xCEFA_EDFE
public let MH_MAGIC_64: UInt32 = 0xFEED_FACF
public let MH_CIGAM_64: UInt32 = 0xCFFA_EDFE
public let FAT_MAGIC: UInt32 = 0xCAFE_BABE
public let FAT_CIGAM: UInt32 = 0xBEBA_FECA

public let LC_SEGMENT: UInt32 = 0x1
public let LC_SEGMENT_64: UInt32 = 0x19
public let LC_SYMTAB: UInt32 = 0x2

public let S_ATTR_DEBUG: UInt32 = 0x0200_0000

// MARK: - Raw struct helpers

public func u32(_ data: Data, _ offset: Int, _ swap: Bool) -> UInt32 {
  let v = data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: offset, as: UInt32.self) }
  return swap ? v.byteSwapped : v
}
public func u16(_ data: Data, _ offset: Int, _ swap: Bool) -> UInt16 {
  let v = data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: offset, as: UInt16.self) }
  return swap ? v.byteSwapped : v
}
public func u64(_ data: Data, _ offset: Int, _ swap: Bool) -> UInt64 {
  let v = data.withUnsafeBytes { $0.loadUnaligned(fromByteOffset: offset, as: UInt64.self) }
  return swap ? v.byteSwapped : v
}
public func put32(_ data: inout Data, _ offset: Int, _ val: UInt32, _ swap: Bool) {
  var v = swap ? val.byteSwapped : val
  withUnsafeBytes(of: &v) { data.replaceSubrange(offset..<offset + 4, with: $0) }
}

// MARK: - Segment name helpers (16-byte fixed C string)

public func segName(_ data: Data, _ offset: Int) -> String {
  let bytes = data[offset..<min(offset + 16, data.count)]
  return String(bytes: bytes.prefix(while: { $0 != 0 }), encoding: .utf8) ?? ""
}

public func writeSegName(_ data: inout Data, _ offset: Int, _ name: String) {
  var buf = [UInt8](repeating: 0, count: 16)
  let nameBytes = Array(name.utf8.prefix(15))
  buf.replaceSubrange(0..<nameBytes.count, with: nameBytes)
  data.replaceSubrange(offset..<offset + 16, with: buf)
}
