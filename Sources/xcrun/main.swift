/// xcrun — shim that maps tool names to system LLVM binaries on Linux.
/// Supports: -sdk <name> -find <tool>, -show-sdk-path, -show-sdk-version,
///           -show-sdk-platform-path, and run mode (xcrun <tool> [args...]).
import Foundation

#if canImport(Glibc)
  import Glibc
#endif

let env = ProcessInfo.processInfo.environment

func envKey(_ tool: String) -> String {
  "XCRUN_" + tool.uppercased().replacingOccurrences(of: "-", with: "_")
}

// Tool → system binary path
let toolMap: [String: String] = [
  "clang": "/usr/bin/clang",
  "clang++": "/usr/bin/clang++",
  "flex": "/usr/bin/flex",
  "bison": "/usr/bin/bison",
  "gm4": "/usr/bin/m4",
  "m4": "/usr/bin/m4",
  "strip": "/usr/bin/llvm-strip",
  "nm": "/usr/bin/llvm-nm",
  "objdump": "/usr/bin/llvm-objdump",
  "otool": "/usr/bin/llvm-otool",
  "lipo": "/usr/bin/llvm-lipo",
  "nmedit": "/usr/bin/llvm-nm",
  "dsymutil": "/usr/bin/llvm-dsymutil",
  "libtool": "/usr/bin/llvm-ar",  // closest equivalent
  "ar": "/usr/bin/llvm-ar",
  "cmake": "/usr/bin/cmake",
  "ninja": "/usr/bin/ninja",
]

// Tools that are no-ops on Linux
let stubTools: Set<String> = [
  "mig", "migcom", "iig", "codesign", "codesign_allocate",
  "ctfconvert", "ctfmerge", "ctfdump", "ctf_insert",
  "scan-build", "docc",
]

func findTool(_ name: String) -> String? {
  if let override = env[envKey(name)] { return override }
  if let path = toolMap[name] { return path }
  if stubTools.contains(name) {
    return env["XCRUN_STUB"]
      ?? (CommandLine.arguments[0]
        .replacingOccurrences(of: "/xcrun", with: "/xcrun-stub"))
  }
  return nil
}

let sdkPath = env["XCRUN_SDK_PATH"] ?? "/"
let sdkVersion = env["XCRUN_SDK_VERSION"] ?? "14.0"
let platformPath = env["XCRUN_PLATFORM_PATH"] ?? "\(sdkPath)/Platforms/MacOSX.platform"

var rawArgs = CommandLine.arguments.dropFirst()[...]

// Skip -sdk <name> (we ignore the SDK name, honour XCRUN_SDK_PATH instead)
if rawArgs.first == "-sdk" { rawArgs = rawArgs.dropFirst(2) }

guard let verb = rawArgs.first else {
  fputs("xcrun: no command specified\n", stderr)
  exit(1)
}
rawArgs = rawArgs.dropFirst()

switch verb {
case "-find":
  guard let tool = rawArgs.first else {
    fputs("xcrun: -find requires a tool name\n", stderr)
    exit(1)
  }
  if let path = findTool(tool) {
    print(path)
  } else {
    fputs("xcrun: \(tool): not found\n", stderr)
    exit(1)
  }

case "-show-sdk-path":
  print(sdkPath)

case "-show-sdk-version":
  print(sdkVersion)

case "-show-sdk-platform-path":
  print(platformPath)

default:
  // Run mode: xcrun <tool> [args...]
  let tool = verb
  guard let binary = findTool(tool) else {
    fputs("xcrun: \(tool): not found\n", stderr)
    exit(1)
  }
  let toolArgs = [binary] + Array(rawArgs)
  let argv = toolArgs.map { $0.withCString(strdup) } + [nil]
  defer { argv.compactMap { $0 }.forEach { free($0) } }
  execv(binary, argv)
  fputs("xcrun: execv \(binary): \(String(cString: strerror(errno)))\n", stderr)
  exit(127)
}
