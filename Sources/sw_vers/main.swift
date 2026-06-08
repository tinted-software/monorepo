/// sw_vers — minimal macOS version reporter shim.
import Foundation

let version = ProcessInfo.processInfo.environment["XCRUN_SDK_VERSION"] ?? "14.0"
let args = CommandLine.arguments.dropFirst()

switch args.first {
case "-productName": print("macOS")
case "-productVersion": print(version)
case "-buildVersion": print("23A344")
default:
  print("ProductName:    macOS")
  print("ProductVersion: \(version)")
  print("BuildVersion:   23A344")
}
