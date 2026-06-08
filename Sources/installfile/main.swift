/// installfile [-c] [-S] [-m <mode>] <src> <dst>
/// Atomically copy src to dst via a temp file + rename.
import Foundation

#if canImport(Glibc)
  import Glibc
#elseif canImport(Darwin)
  import Darwin
#endif

var mode: mode_t = 0o644
var gotMode = false
var src: String?
var dst: String?

var args = CommandLine.arguments.dropFirst()
while let arg = args.first {
  args = args.dropFirst()
  switch arg {
  case "-c", "-S": break  // compatibility flags, ignored
  case "-m":
    guard let modeStr = args.first else {
      fputs("installfile: -m requires an argument\n", stderr)
      exit(1)
    }
    args = args.dropFirst()
    guard let parsed = mode_t(modeStr, radix: 8) else {
      fputs("installfile: bad mode \(modeStr)\n", stderr)
      exit(1)
    }
    mode = parsed
    gotMode = true
  default:
    if src == nil {
      src = arg
    } else if dst == nil {
      dst = arg
    } else {
      fputs("installfile: unexpected argument \(arg)\n", stderr)
      exit(1)
    }
  }
}

guard let src, let dst else {
  fputs("Usage: installfile [-c] [-S] [-m <mode>] <src> <dst>\n", stderr)
  exit(1)
}

let fm = FileManager.default

// Read source
guard let data = fm.contents(atPath: src) else {
  fputs("installfile: cannot read \(src)\n", stderr)
  exit(1)
}

// Write to temp file next to dst, then rename
let pid = ProcessInfo.processInfo.processIdentifier
let tmp = dst + ".XXXXXX.\(pid)"

do {
  try data.write(to: URL(fileURLWithPath: tmp), options: [])
  if gotMode {
    try fm.setAttributes([.posixPermissions: mode], ofItemAtPath: tmp)
  }
  // Atomic replace
  if fm.fileExists(atPath: dst) {
    _ = try fm.replaceItemAt(URL(fileURLWithPath: dst), withItemAt: URL(fileURLWithPath: tmp))
  } else {
    try fm.moveItem(atPath: tmp, toPath: dst)
  }
} catch {
  try? fm.removeItem(atPath: tmp)
  fputs("installfile: \(error)\n", stderr)
  exit(1)
}
