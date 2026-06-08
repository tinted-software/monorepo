/// setsegname [-s OLDSEGNAME] [-i IGNORESEGNAME] -n NEWSEGNAME <input> -o <output>
/// Renames Mach-O segment names in all section headers.
import Foundation
import MachOKit

var oldSeg: String?
var ignoreSeg: String?
var newSeg: String?
var input: String?
var output: String?

var args = CommandLine.arguments.dropFirst()[...]
while let a = args.first {
  args = args.dropFirst()
  switch a {
  case "-s":
    oldSeg = args.first
    args = args.dropFirst()
  case "-i":
    ignoreSeg = args.first
    args = args.dropFirst()
  case "-n":
    newSeg = args.first
    args = args.dropFirst()
  case "-o":
    output = args.first
    args = args.dropFirst()
  default: input = a
  }
}
guard let newSeg, let input, let output else {
  fputs("Usage: setsegname [-s OLD] [-i IGNORE] -n NEW <input> -o <output>\n", stderr)
  exit(1)
}

guard var data = FileManager.default.contents(atPath: input) else {
  fputs("setsegname: cannot read \(input)\n", stderr)
  exit(1)
}

let magic = u32(data, 0, false)
let swap: Bool
let is64: Bool
let ncmdsOff: Int
let cmdStart: Int

switch magic {
case MachOKit.MH_MAGIC:
  swap = false
  is64 = false
  ncmdsOff = 16
  cmdStart = 28
case MachOKit.MH_CIGAM:
  swap = true
  is64 = false
  ncmdsOff = 16
  cmdStart = 28
case MachOKit.MH_MAGIC_64:
  swap = false
  is64 = true
  ncmdsOff = 16
  cmdStart = 32
case MachOKit.MH_CIGAM_64:
  swap = true
  is64 = true
  ncmdsOff = 16
  cmdStart = 32
default:
  fputs("setsegname: not a Mach-O file\n", stderr)
  exit(1)
}

var ncmds = u32(data, ncmdsOff, swap)
var off = cmdStart

for _ in 0..<ncmds {
  let cmd = u32(data, off, swap)
  let cmdsize = u32(data, off + 4, swap)
  let sectSize = is64 ? 80 : 68
  let sectBase = off + (is64 ? 64 : 56)
  let nsects: UInt32

  if cmd == MachOKit.LC_SEGMENT || cmd == MachOKit.LC_SEGMENT_64 {
    nsects = u32(data, off + (is64 ? 56 : 48), swap)
    for s in 0..<nsects {
      let sectOff = sectBase + Int(s) * sectSize
      let flags = u32(data, sectOff + (is64 ? 64 : 56), swap)
      if flags & MachOKit.S_ATTR_DEBUG != 0 { continue }
      let sname = segName(data, sectOff + 16)  // section's segname field
      if let ig = ignoreSeg, ig == sname { continue }
      if oldSeg == nil || oldSeg == sname {
        writeSegName(&data, sectOff + 16, newSeg)
      }
    }
  }
  off += Int(cmdsize)
}

do {
  try data.write(to: URL(fileURLWithPath: output))
} catch {
  fputs("setsegname: write failed: \(error)\n", stderr)
  exit(1)
}
