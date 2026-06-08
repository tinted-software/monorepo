import Foundation

// 1. Parse command line arguments
var cpu = "x86_64"
var soc = "none"
var platform = "MacOSX"
var outputDir = ""
var sourceDir = ""
var masterDir = ""
var configName = "DEVELOPMENT"

var args = CommandLine.arguments.dropFirst()[...]
while let arg = args.first {
  args = args.dropFirst()
  switch arg {
  case "-cpu":
    cpu = args.first ?? ""
    args = args.dropFirst()
  case "-soc":
    soc = args.first ?? ""
    args = args.dropFirst()
  case "-platform":
    platform = args.first ?? ""
    args = args.dropFirst()
  case "-d":
    outputDir = args.first ?? ""
    args = args.dropFirst()
  case "-s":
    sourceDir = args.first ?? ""
    args = args.dropFirst()
  case "-m":
    masterDir = args.first ?? ""
    args = args.dropFirst()
  default:
    if !arg.hasPrefix("-") {
      configName = arg
    }
  }
}

guard !outputDir.isEmpty, !sourceDir.isEmpty, !masterDir.isEmpty else {
  fputs(
    "Usage: config -cpu <cpu> -soc <soc> -platform <platform> -d <output_dir> -s <source_dir> -m <master_dir> <config_name>\n",
    stderr)
  exit(1)
}

// 2. Preprocess MASTER and MASTER.<cpu> to collect options and configurations
let masterFile = "\(masterDir)/MASTER"
let cpuMasterFile = "\(masterDir)/MASTER.\(cpu)"
let filesToPreprocess = [masterFile, cpuMasterFile].filter {
  FileManager.default.fileExists(atPath: $0)
}

print("Config: Reading configurations from \(filesToPreprocess)")

// 2.1 First pass: collect config definitions
var configDefs: [String: [String]] = [:]
let configDefRegex = try! NSRegularExpression(pattern: #"^\s*#\s*([A-Za-z0-9_]+)\s*=\s*\[(.*)\]"#)

for file in filesToPreprocess {
  guard let content = try? String(contentsOfFile: file, encoding: .utf8) else { continue }
  let lines = content.components(separatedBy: .newlines)
  for line in lines {
    let nsLine = line as NSString
    if let match = configDefRegex.firstMatch(
      in: line, options: [], range: NSRange(location: 0, length: nsLine.length))
    {
      let name = nsLine.substring(with: match.range(at: 1))
      let valStr = nsLine.substring(with: match.range(at: 2))
      let attrs = valStr.components(separatedBy: .whitespaces).map {
        $0.trimmingCharacters(in: .whitespacesAndNewlines)
      }.filter { !$0.isEmpty }
      configDefs[name] = attrs
    }
  }
}

// 2.2 Recursively resolve config attributes
var activeAttrs = Set<String>()
@MainActor
func resolveConfig(_ name: String) {
  guard let list = configDefs[name] else {
    activeAttrs.insert(name.lowercased())
    return
  }
  for item in list {
    if configDefs[item] != nil {
      resolveConfig(item)
    } else {
      activeAttrs.insert(item.lowercased())
    }
  }
}
resolveConfig(configName)

// Predefine standard features
activeAttrs.insert(cpu.lowercased())
activeAttrs.insert(platform.lowercased())
activeAttrs.insert(soc.lowercased())
activeAttrs.insert(configName.lowercased())

print("Config: Active attributes: \(activeAttrs.sorted())")

// 2.3 Second pass: evaluate preprocessor directives (#ifdef) and attribute selectors
var enabledOptions = Set<String>()
var enabledDevices = Set<String>()
var allKnownDevices = Set<String>()
var allKnownOptions = Set<String>()

@MainActor
func evaluateCondition(_ cond: String) -> Bool {
  let trimmed = cond.trimmingCharacters(in: .whitespacesAndNewlines)
  if trimmed == "CPU_\(cpu)" || trimmed == "PLATFORM_\(platform)" || trimmed == "SYS_\(configName)"
    || trimmed == "SOC_CONFIG_\(soc)"
  {
    return true
  }
  if trimmed.hasPrefix("CPU_") || trimmed.hasPrefix("PLATFORM_") || trimmed.hasPrefix("SOC_CONFIG_")
  {
    return false
  }
  return activeAttrs.contains(trimmed.lowercased())
}

for file in filesToPreprocess {
  guard let content = try? String(contentsOfFile: file, encoding: .utf8) else { continue }
  let lines = content.components(separatedBy: .newlines)

  var condStack: [Bool] = [true]

  for line in lines {
    let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)

    if trimmed.hasPrefix("#") {
      let parts = trimmed.dropFirst().components(separatedBy: .whitespaces).filter { !$0.isEmpty }
      if let directive = parts.first {
        if directive == "ifdef" {
          let cond = parts.dropFirst().joined(separator: " ")
          condStack.append(condStack.last! && evaluateCondition(cond))
          continue
        } else if directive == "ifndef" {
          let cond = parts.dropFirst().joined(separator: " ")
          condStack.append(condStack.last! && !evaluateCondition(cond))
          continue
        } else if directive == "if" {
          let cond = parts.dropFirst().joined(separator: " ")
          condStack.append(condStack.last! && evaluateCondition(cond))
          continue
        } else if directive == "else" {
          if condStack.count > 1 {
            let parentCond = condStack[condStack.count - 2]
            let currentCond = condStack.last!
            condStack[condStack.count - 1] = parentCond && !currentCond
          }
          continue
        } else if directive == "endif" {
          if condStack.count > 1 {
            condStack.removeLast()
          }
          continue
        }
      }
    }

    guard condStack.last! else { continue }
    if trimmed.hasPrefix("#") || trimmed.isEmpty { continue }

    var lineContent = line
    var selector: String? = nil

    if let lastHashIdx = line.lastIndex(of: "#") {
      let tail = line[line.index(after: lastHashIdx)...].trimmingCharacters(
        in: .whitespacesAndNewlines)
      if tail.hasPrefix("<") && tail.hasSuffix(">") {
        selector = String(tail.dropFirst().dropLast())
        lineContent = String(line[..<lastHashIdx])
      }
    }

    var isSelected = true
    if let sel = selector {
      let isNegated = sel.hasPrefix("!")
      let attrsStr = isNegated ? String(sel.dropFirst()) : sel
      let attrs = attrsStr.components(separatedBy: ",").map {
        $0.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
      }
      let hasOverlap = !activeAttrs.intersection(attrs).isEmpty
      isSelected = isNegated ? !hasOverlap : hasOverlap
    }

    let parts = lineContent.components(separatedBy: .whitespaces).filter { !$0.isEmpty }
    guard parts.count >= 2 else { continue }
    let type = parts[0].lowercased()
    let name = parts[1]

    if type == "options" {
      let optName = name.components(separatedBy: "=")[0]
      allKnownOptions.insert(optName)
      if isSelected { enabledOptions.insert(optName) }
    } else if type == "device" || type == "pseudo-device" {
      allKnownDevices.insert(name)
      if isSelected { enabledDevices.insert(name) }
    }
  }
}

// 3. Read files and files.<cpu> to collect needed header dependencies
var neededHeaders = Set<String>()
let filesPaths = ["\(sourceDir)/files", "\(sourceDir)/files.\(cpu)"].filter {
  FileManager.default.fileExists(atPath: $0)
}

for file in filesPaths {
  guard let content = try? String(contentsOfFile: file, encoding: .utf8) else { continue }
  let lines = content.components(separatedBy: .newlines)
  for line in lines {
    let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.hasPrefix("#") || trimmed.isEmpty { continue }

    let parts = trimmed.components(separatedBy: .whitespaces).filter { !$0.isEmpty }
    if let idx = parts.firstIndex(where: { $0 == "optional" || $0 == "needs-header" }) {
      let opts = parts[(idx + 1)...]
      for opt in opts {
        let cleaned = opt.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        if !cleaned.isEmpty {
          neededHeaders.insert(cleaned)
        }
      }
    }
  }
}

// Always generate headers for all known options and devices from MASTER as well
for opt in allKnownOptions { neededHeaders.insert(opt) }
for dev in allKnownDevices { neededHeaders.insert(dev) }

// Add some extra headers that are always needed by XNU
neededHeaders.insert("mach_ldebug")
neededHeaders.insert("mach_assert")

// 4. Generate the header files
try! FileManager.default.createDirectory(atPath: outputDir, withIntermediateDirectories: true)

for name in neededHeaders {
  let headerFile = "\(outputDir)/\(name.lowercased()).h"
  let macroName: String
  let value: Int

  if allKnownDevices.contains(name) {
    macroName = "N" + name.uppercased()
    value = enabledDevices.contains(name) ? 1 : 0
  } else {
    macroName = name.uppercased()
    value = enabledOptions.contains(name) ? 1 : 0
  }

  let content = "#define \(macroName) \(value)\n"

  // Write only if different to prevent rebuilding
  if let existing = try? String(contentsOfFile: headerFile, encoding: .utf8), existing == content {
    // no-op
  } else {
    try! content.write(toFile: headerFile, atomically: true, encoding: .utf8)
  }
}

// 5. Generate meta_features.h
var metaFeaturesContent = ""
for headerName in neededHeaders.sorted() {
  metaFeaturesContent += "#include <\(headerName.lowercased()).h>\n"
}
let metaFeaturesFile = "\(outputDir)/meta_features.h"
if let existing = try? String(contentsOfFile: metaFeaturesFile, encoding: .utf8),
  existing == metaFeaturesContent
{
  // no-op
} else {
  try! metaFeaturesContent.write(toFile: metaFeaturesFile, atomically: true, encoding: .utf8)
}

print("Config: Generated \(neededHeaders.count) header files in \(outputDir)")
