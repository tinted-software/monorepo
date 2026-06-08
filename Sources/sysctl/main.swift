/// sysctl — shim for hw.memsize, hw.physicalcpu, hw.logicalcpu on Linux.
import Foundation

#if canImport(Glibc)
  import Glibc
#endif

let args = CommandLine.arguments.dropFirst()

// Only handle: sysctl -n <key>
guard args.first == "-n", let key = args.dropFirst().first else {
  fputs("sysctl: usage: sysctl -n <key>\n", stderr)
  exit(1)
}

func cpuCount() -> Int {
  return ProcessInfo.processInfo.processorCount
}

func memSize() -> UInt64 {
  return ProcessInfo.processInfo.physicalMemory
}

switch key {
case "hw.memsize":
  print(memSize())
case "hw.physicalcpu", "hw.logicalcpu",
  "hw.physicalcpu_max", "hw.logicalcpu_max":
  print(cpuCount())
default:
  fputs("sysctl: unknown key \(key)\n", stderr)
  exit(1)
}
