// main.swift — Pure Swift Mach Interface Generator entry point

import Foundation

// MARK: - Argument parsing

var isPreprocess = false
var inputFile: String? = nil

@MainActor
func parseArgs() {
  var args = CommandLine.arguments.dropFirst()

  if args.first == "-version" {
    print(Global.MIG_VERSION)
    exit(0)
  }

  while let arg = args.first {
    if arg.hasPrefix("-") {
      switch arg {
      case "-q":
        Global.beQuiet = true
        args.removeFirst()
      case "-Q":
        Global.beQuiet = false
        args.removeFirst()
      case "-v":
        Global.beVerbose = true
        args.removeFirst()
      case "-V":
        Global.beVerbose = false
        args.removeFirst()
      case "-r":
        Global.useMsgRPC = true
        args.removeFirst()
      case "-R":
        Global.useMsgRPC = false
        args.removeFirst()
      case "-k":
        Global.beAnsiC = true
        args.removeFirst()
      case "-K":
        Global.beAnsiC = false
        args.removeFirst()
      case "-n", "-novouchers":
        if arg == "-novouchers" {
          Global.isVoucherCodeAllowed = false
        } else {
          Global.checkNDR = true
        }
        args.removeFirst()
      case "-N":
        Global.checkNDR = false
        args.removeFirst()
      case "-s":
        Global.genSymTab = true
        args.removeFirst()
      case "-S":
        Global.genSymTab = false
        args.removeFirst()
      case "-l":
        Global.useEventLogger = false
        args.removeFirst()
      case "-L":
        Global.useEventLogger = true
        args.removeFirst()
      case "-split":
        Global.useSplitHeaders = true
        args.removeFirst()
      case "-mach_msg2":
        Global.useMachMsg2 = true
        args.removeFirst()
      case "-b":
        Global.emitCountAnnotations = true
        args.removeFirst()
      case "-B":
        Global.emitCountAnnotations = false
        args.removeFirst()

      case "-user":
        args.removeFirst()
        if let name = args.first {
          Global.userFileName = name
          args.removeFirst()
        } else {
          fatal("missing name for -user option")
        }

      case "-server":
        args.removeFirst()
        if let name = args.first {
          Global.serverFileName = name
          args.removeFirst()
        } else {
          fatal("missing name for -server option")
        }

      case "-header":
        args.removeFirst()
        if let name = args.first {
          Global.userHeaderFileName = name
          args.removeFirst()
        } else {
          fatal("missing name for -header option")
        }

      case "-sheader":
        args.removeFirst()
        if let name = args.first {
          Global.serverHeaderFileName = name
          args.removeFirst()
        } else {
          fatal("missing name for -sheader option")
        }

      case "-iheader":
        args.removeFirst()
        if let name = args.first {
          Global.internalHeaderFileName = name
          args.removeFirst()
        } else {
          fatal("missing name for -iheader option")
        }

      case "-dheader":
        args.removeFirst()
        if let name = args.first {
          Global.definesHeaderFileName = name
          args.removeFirst()
        } else {
          fatal("missing name for -dheader option")
        }

      case "-types-header":
        args.removeFirst()
        if let name = args.first {
          Global.typesHeaderFileName = name
          args.removeFirst()
        } else {
          fatal("missing name for -types-header option")
        }
        Global.typesOnlyMode = true

      case "-i":
        args.removeFirst()
        if let prefix = args.first {
          Global.userFilePrefix = prefix
          args.removeFirst()
        } else {
          fatal("missing prefix for -i option")
        }

      case "-maxonstack":
        args.removeFirst()
        if let size = args.first, let n = Int(size) {
          Global.maxMessSizeOnStack = n
          args.removeFirst()
        } else {
          fatal("missing size for -maxonstack option")
        }

      case "-max_descrs":
        args.removeFirst()
        if let cnt = args.first, let n = Int(cnt) {
          Global.maxServerDescrs = n
          args.removeFirst()
        } else {
          fatal("missing count for -max_descrs option")
        }

      case "-max_reply_descrs":
        args.removeFirst()
        if let cnt = args.first, let n = Int(cnt) {
          Global.maxServerReplyDescrs = n
          args.removeFirst()
        } else {
          fatal("missing count for -max_reply_descrs option")
        }

      // Preprocessor flags that we ignore but accept
      case "-E", "-D", "-I", "-U", "-isysroot", "-arch", "-target",
        "-MD", "-MF", "-MT", "-MQ":
        // Skip flag and its argument
        args.removeFirst()
        if let next = args.first, !next.hasPrefix("-") { args.removeFirst() }

      default:
        if arg.hasPrefix("-D") || arg.hasPrefix("-I") || arg.hasPrefix("-U") {
          args.removeFirst()
        } else {
          fatal("unknown flag: '\(arg)'")
        }
      }
    } else {
      inputFile = arg
      args.removeFirst()
      break
    }
  }
}

// MARK: - Main

setProgramName("mig")
parseArgs()

guard let inputPath = inputFile else {
  fatal("No input file specified")
}

// Read input file
let input: String
do {
  input = try String(contentsOfFile: inputPath, encoding: .utf8)
} catch {
  // Try running through clang preprocessor
  isPreprocess = true
  input = preprocessFile(inputPath)
}

Global.initGlobal()
TypeSystem.initType()

// Lex
let lexer = Lexer(input: input)
yyinname = inputPath

// Parse
let tokenStream = TokenStream(lexer: lexer)
let parser = Parser(tokens: tokenStream)
let statements = parser.parse()

if migErrors > 0 {
  fatal("\(migErrors) errors found. Abort.")
}

Global.moreGlobal()

// Generate output files
if Global.beVerbose {
  if !Global.userHeaderFileName.isEmpty {
    print("Writing \(Global.userHeaderFileName) ... ", terminator: "")
  }
}
if !Global.userHeaderFileName.isEmpty {
  writeUserHeader(fileName: Global.userHeaderFileName, statements: statements)
  if Global.beVerbose { print("done.") }
}

if !Global.serverHeaderFileName.isEmpty {
  if Global.beVerbose { print("Writing \(Global.serverHeaderFileName) ... ", terminator: "") }
  writeServerHeader(fileName: Global.serverHeaderFileName, statements: statements)
  if Global.beVerbose { print("done.") }
}

if Global.isKernelServer && !Global.internalHeaderFileName.isEmpty {
  if Global.beVerbose { print("Writing \(Global.internalHeaderFileName) ... ", terminator: "") }
  writeInternalHeader(fileName: Global.internalHeaderFileName, statements: statements)
  if Global.beVerbose { print("done.") }
}

if !Global.definesHeaderFileName.isEmpty {
  if Global.beVerbose { print("Writing \(Global.definesHeaderFileName) ... ", terminator: "") }
  writeDefinesHeader(fileName: Global.definesHeaderFileName, statements: statements)
  if Global.beVerbose { print("done.") }
}

if !Global.userFileName.isEmpty {
  if Global.beVerbose { print("Writing \(Global.userFileName) ... ", terminator: "") }
  writeUser(fileName: Global.userFileName, statements: statements)
  if Global.beVerbose { print("done.") }
}

if !Global.serverFileName.isEmpty {
  if Global.beVerbose { print("Writing \(Global.serverFileName) ... ", terminator: "") }
  writeServer(fileName: Global.serverFileName, statements: statements)
  if Global.beVerbose { print("done.") }
}

// Generate individual user files if -i was given
if !Global.userFilePrefix.isEmpty {
  let routineStatements = statements.filter { $0.stKind == .routine }
  for st in routineStatements {
    guard let rt = st.stRoutine else { continue }
    let fileName = "\(Global.userFilePrefix)\(rt.rtName).c"
    if Global.beVerbose { print("Writing \(fileName) ... ", terminator: "") }
    writeUser(fileName: fileName, statements: [st])
    if Global.beVerbose { print("done.") }
  }
}

if Global.beVerbose { print("") }

// Types-only mode: write just the typedef header and stop.
if Global.typesOnlyMode {
  if !Global.typesHeaderFileName.isEmpty {
    if Global.beVerbose {
      print("Writing types header \(Global.typesHeaderFileName) ... ", terminator: "")
    }
    writeTypesHeader(fileName: Global.typesHeaderFileName)
    if Global.beVerbose { print("done.") }
  }
  exit(0)
}

// MARK: - Preprocessing support

func preprocessFile(_ path: String) -> String {
  // Run clang -E on the file
  let task = Process()
  task.executableURL = URL(fileURLWithPath: "/usr/bin/clang")
  task.arguments = ["-E", "-D__MACH30__", path]

  let pipe = Pipe()
  task.standardOutput = pipe
  task.standardError = FileHandle.standardError

  do {
    try task.run()
    task.waitUntilExit()

    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    if let output = String(data: data, encoding: .utf8) {
      return output
    }
  } catch {
    fatal("Cannot preprocess: \(error)")
  }

  fatal("Preprocessing failed")
}
