/// replacecontents <dst> <new> [<content> ...]
/// Atomically replace <dst> with <new> only if contents differ.
/// Remaining args are additional files whose content is compared.
import Foundation

let args = CommandLine.arguments
guard args.count >= 3 else {
  fputs("Usage: replacecontents <dst> <new> [<contents> ...]\n", stderr)
  exit(1)
}

let dst = args[1]
let new = args[2]

let fm = FileManager.default

@MainActor
func readFile(_ path: String) -> Data? {
  return fm.contents(atPath: path)
}

let newData = readFile(new) ?? Data()

// Check if dst exists and matches
if let existing = readFile(dst), existing == newData {
  exit(0)  // Contents identical — do nothing
}

// Atomically replace dst with new
do {
  let tmpURL = URL(fileURLWithPath: dst + ".tmp.\(ProcessInfo.processInfo.processIdentifier)")
  try newData.write(to: tmpURL, options: .atomic)
  _ = try fm.replaceItemAt(URL(fileURLWithPath: dst), withItemAt: tmpURL)
} catch {
  // Try simple rename if replaceItemAt fails (dst may not exist yet)
  do {
    let tmp = dst + ".tmp.\(ProcessInfo.processInfo.processIdentifier)"
    try newData.write(to: URL(fileURLWithPath: tmp), options: .atomic)
    try fm.moveItem(atPath: tmp, toPath: dst)
  } catch {
    fputs("replacecontents: \(error)\n", stderr)
    exit(1)
  }
}
