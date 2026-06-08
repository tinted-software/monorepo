#!/usr/bin/env swift
/// xnu-sdk-gen — Generate a Darwin kernel SDK from an XNU source tree.
///
/// Usage:
///   xnu-sdk-gen --xnu <path-to-xnu> --output <path-to-sdk> [--clean] [--verbose]
///
/// The script mirrors the header install layout that Apple's XNU Makefiles
/// produce, copying public headers into a sysroot-style tree:
///
///   <sdk>/usr/include/
///     mach/          ← osfmk/mach/*.h
///     mach/i386/     ← osfmk/mach/i386/*.h
///     mach/arm/      ← osfmk/mach/arm/*.h
///     machine/       ← osfmk/machine/*.h  (arch-generic wrappers)
///     sys/           ← bsd/sys/*.h
///     net/           ← bsd/net/*.h
///     netinet/       ← bsd/netinet/*.h
///     netinet6/      ← bsd/netinet6/*.h
///     bsm/           ← bsd/bsm/*.h
///     security/      ← bsd/security/*.h
///     uuid/          ← bsd/uuid/*.h
///     libkern/       ← libkern/*.h
///     IOKit/         ← iokit/IOKit/*.h
///     pexpert/       ← pexpert/pexpert/*.h
///     architecture/  ← EXTERNAL_HEADERS/architecture/*.h
///     mach-o/        ← EXTERNAL_HEADERS/mach-o/*.h
///     corecrypto/    ← EXTERNAL_HEADERS/corecrypto/*.h
///     (root files)   ← EXTERNAL_HEADERS/*.h

import Foundation

// MARK: - CLI Parsing

struct Config {
  var xnuRoot: URL
  var sdkRoot: URL
  var clean: Bool = false
  var verbose: Bool = false
}

func usage() -> Never {
  fputs(
    """
    Usage: xnu-sdk-gen --xnu <xnu-dir> --output <sdk-dir> [--clean] [--verbose]

    Options:
      --xnu <path>     Path to the XNU source tree root  (required)
      --output <path>  Path to the SDK output directory  (required)
      --clean          Remove and recreate output before copying
      --verbose        Print each installed header
      --help           Show this message
    """, stderr)
  exit(1)
}

func parseArgs() -> Config {
  var args = CommandLine.arguments.dropFirst()
  var xnu: String? = nil
  var output: String? = nil
  var clean = false
  var verbose = false

  while !args.isEmpty {
    let arg = args.removeFirst()
    switch arg {
    case "--xnu":
      guard !args.isEmpty else {
        fputs("--xnu requires a value\n", stderr)
        exit(1)
      }
      xnu = args.removeFirst()
    case "--output":
      guard !args.isEmpty else {
        fputs("--output requires a value\n", stderr)
        exit(1)
      }
      output = args.removeFirst()
    case "--clean": clean = true
    case "--verbose": verbose = true
    case "--help": usage()
    default:
      fputs("Unknown argument: \(arg)\n", stderr)
      usage()
    }
  }

  guard let x = xnu, let o = output else {
    fputs("--xnu and --output are required\n", stderr)
    usage()
  }

  return Config(
    xnuRoot: URL(fileURLWithPath: x).standardized,
    sdkRoot: URL(fileURLWithPath: o).standardized,
    clean: clean,
    verbose: verbose
  )
}

// MARK: - File System Helpers

let fm = FileManager.default

@MainActor
func mkdir(_ url: URL) throws {
  try fm.createDirectory(at: url, withIntermediateDirectories: true)
}

/// Copy every *.h (and optionally *.modulemap) from `src` into `dst`,
/// preserving relative subdirectory structure if `recursive` is true.
@discardableResult
@MainActor
func installHeaders(
  from src: URL,
  to dst: URL,
  recursive: Bool = false,
  extensions: [String] = ["h"],
  verbose: Bool = false
) throws -> Int {
  guard fm.fileExists(atPath: src.path) else { return 0 }

  var count = 0
  let opts: FileManager.DirectoryEnumerationOptions =
    recursive ? [] : [.skipsSubdirectoryDescendants]
  guard
    let enumerator = fm.enumerator(
      at: src, includingPropertiesForKeys: [.isRegularFileKey], options: opts)
  else {
    return 0
  }

  for case let fileURL as URL in enumerator {
    guard extensions.contains(fileURL.pathExtension) else { continue }

    // Compute relative path from src
    let rel = fileURL.path.dropFirst(src.path.count)
      .drop(while: { $0 == "/" })
    let destFile = dst.appendingPathComponent(String(rel))

    try mkdir(destFile.deletingLastPathComponent())

    if fm.fileExists(atPath: destFile.path) {
      try fm.removeItem(at: destFile)
    }
    try fm.copyItem(at: fileURL, to: destFile)

    if verbose {
      print("  install: \(rel)")
    }
    count += 1
  }
  return count
}

// MARK: - Main Logic

let cfg = parseArgs()
let xnu = cfg.xnuRoot
let sdk = cfg.sdkRoot.appendingPathComponent("usr/include")

print("xnu-sdk-gen")
print("  XNU root : \(xnu.path)")
print("  SDK root : \(cfg.sdkRoot.path)")
print("")

// --clean: wipe the include tree
if cfg.clean && fm.fileExists(atPath: sdk.path) {
  print("[clean] Removing \(sdk.path)...")
  try fm.removeItem(at: sdk)
}
try mkdir(sdk)

var totalInstalled = 0

/// Helper closure to install a subtree and print a summary line.
@MainActor
func install(label: String, from src: URL, to dst: URL, recursive: Bool = false) throws {
  let n = try installHeaders(from: src, to: dst, recursive: recursive, verbose: cfg.verbose)
  totalInstalled += n
  print("[\(String(format: "%4d", n)) headers] \(label)")
}

// ─────────────────────────────────────────────────
// osfmk — Mach interfaces
// ─────────────────────────────────────────────────
let osfmk = xnu.appendingPathComponent("osfmk")

// mach/* (public Mach API headers, with arch subdirs)
try install(
  label: "mach/",
  from: osfmk.appendingPathComponent("mach"),
  to: sdk.appendingPathComponent("mach"),
  recursive: true
)

// machine/ (arch-neutral wrappers that #include the real arch headers)
try install(
  label: "machine/  [osfmk/machine]",
  from: osfmk.appendingPathComponent("machine"),
  to: sdk.appendingPathComponent("machine")
)

// kern/ private-but-needed subset (not exported in Apple's SDK, kept here for
// kernel-extension developers who need kern/assert.h etc.)
try install(
  label: "kern/     [osfmk/kern — public subset]",
  from: osfmk.appendingPathComponent("kern"),
  to: sdk.appendingPathComponent("kern")
)

// ─────────────────────────────────────────────────
// bsd — BSD interfaces
// ─────────────────────────────────────────────────
let bsd = xnu.appendingPathComponent("bsd")

let bsdSubdirs: [(String, String)] = [
  ("sys", "sys"),
  ("net", "net"),
  ("netinet", "netinet"),
  ("netinet6", "netinet6"),
  ("netkey", "netkey"),
  ("bsm", "bsm"),
  ("security", "security"),
  ("uuid", "uuid"),
  ("machine", "machine"),  // bsd/machine supplements osfmk/machine
  ("i386", "i386"),  // bsd/i386 supplements osfmk/i386
]

for (src, dst) in bsdSubdirs {
  try install(
    label: "\(dst)/     [bsd/\(src)]",
    from: bsd.appendingPathComponent(src),
    to: sdk.appendingPathComponent(dst),
    recursive: true
  )
}

// ─────────────────────────────────────────────────
// libkern
// ─────────────────────────────────────────────────
try install(
  label: "libkern/  [libkern]",
  from: xnu.appendingPathComponent("libkern"),
  to: sdk.appendingPathComponent("libkern"),
  recursive: true
)

// ─────────────────────────────────────────────────
// IOKit
// ─────────────────────────────────────────────────
try install(
  label: "IOKit/    [iokit/IOKit]",
  from: xnu.appendingPathComponent("iokit/IOKit"),
  to: sdk.appendingPathComponent("IOKit"),
  recursive: true
)

// ─────────────────────────────────────────────────
// pexpert
// ─────────────────────────────────────────────────
try install(
  label: "pexpert/  [pexpert/pexpert]",
  from: xnu.appendingPathComponent("pexpert/pexpert"),
  to: sdk.appendingPathComponent("pexpert"),
  recursive: true
)

// ─────────────────────────────────────────────────
// EXTERNAL_HEADERS — Availability, architecture, corecrypto, mach-o, etc.
// ─────────────────────────────────────────────────
let ext = xnu.appendingPathComponent("EXTERNAL_HEADERS")

// Top-level *.h files (Availability.h, AssertMacros.h, …)
try install(
  label: "(root)    [EXTERNAL_HEADERS/*.h]",
  from: ext,
  to: sdk
)

let extSubdirs: [(String, String)] = [
  ("architecture", "architecture"),
  ("corecrypto", "corecrypto"),
  ("mach-o", "mach-o"),
  ("CXX", "CXX"),
]

for (src, dst) in extSubdirs {
  try install(
    label: "\(dst)/  [EXTERNAL_HEADERS/\(src)]",
    from: ext.appendingPathComponent(src),
    to: sdk.appendingPathComponent(dst),
    recursive: true
  )
}

// ─────────────────────────────────────────────────
// prng — random.h and entropy.h (used by kernel extensions)
// ─────────────────────────────────────────────────
try install(
  label: "prng/     [osfmk/prng]",
  from: osfmk.appendingPathComponent("prng"),
  to: sdk.appendingPathComponent("prng")
)

// ─────────────────────────────────────────────────
// Summary
// ─────────────────────────────────────────────────
print("")
print("Done. Installed \(totalInstalled) headers into \(cfg.sdkRoot.path)")
