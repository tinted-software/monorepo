// swift-tools-version: 6.4
// The swift-tools-version declares the minimum version of Swift required to build this package.

import PackageDescription

let package = Package(
  name: "OpenDarwinTools",
  defaultLocalization: "en",
  platforms: [
    .iOS(.v26),
    .macOS(.v26),
  ],
  products: [
    .library(
      name: "MachOKit",
      targets: ["MachOKit"]
    ),
    .executable(
      name: "config",
      targets: ["config"]
    ),
    .executable(
      name: "decomment",
      targets: ["decomment"]
    ),
    .executable(
      name: "installfile",
      targets: ["installfile"]
    ),
    .executable(
      name: "replacecontents",
      targets: ["replacecontents"]
    ),
    .executable(
      name: "setsegname",
      targets: ["setsegname"]
    ),
    .executable(
      name: "sw_vers",
      targets: ["sw_vers"]
    ),
    .executable(
      name: "sysctl",
      targets: ["sysctl"]
    ),
    .executable(
      name: "xcrun",
      targets: ["xcrun"]
    ),
    .executable(
      name: "xnu-sdk-gen",
      targets: ["xnu-sdk-gen"]
    ),
  ],
  dependencies: [],
  targets: [
    .target(
      name: "MachOKit",
      dependencies: [],
      path: "Sources/MachO"
    ),
    .executableTarget(
      name: "config",
      dependencies: [],
      path: "Sources/config"
    ),
    .executableTarget(
      name: "decomment",
      dependencies: [],
      path: "Sources/decomment"
    ),
    .executableTarget(
      name: "installfile",
      dependencies: [],
      path: "Sources/installfile"
    ),
    .executableTarget(
      name: "replacecontents",
      dependencies: [],
      path: "Sources/replacecontents"
    ),
    .executableTarget(
      name: "setsegname",
      dependencies: ["MachOKit"],
      path: "Sources/setsegname"
    ),
    .executableTarget(
      name: "sw_vers",
      dependencies: [],
      path: "Sources/sw_vers"
    ),
    .executableTarget(
      name: "sysctl",
      dependencies: [],
      path: "Sources/sysctl"
    ),
    .executableTarget(
      name: "xcrun",
      dependencies: [],
      path: "Sources/xcrun"
    ),
    .executableTarget(
      name: "xnu-sdk-gen",
      dependencies: [],
      path: "Sources/xnu-sdk-gen"
    ),
  ]
)
