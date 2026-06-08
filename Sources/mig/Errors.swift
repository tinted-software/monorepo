// Errors.swift — Error handling for the Mach Interface Generator

import Foundation

nonisolated(unsafe) var programName = "mig"
nonisolated(unsafe) var migErrors = 0
nonisolated(unsafe) var lineno = 0
nonisolated(unsafe) var yyinname = "<no name yet>"

func setProgramName(_ name: String) {
  programName = name
}

func fatal(_ format: String, _ args: CVarArg...) -> Never {
  let msg: String
  if args.isEmpty {
    msg = format
  } else {
    msg = String(format: format, arguments: args)
  }
  FileHandle.standardError.write(
    Data("\(programName): fatal: \"\(yyinname)\", line \(lineno): \(msg)\n".utf8))
  exit(1)
}

func warning(_ format: String, _ args: CVarArg...) {
  guard !Global.beQuiet, migErrors == 0 else { return }
  let msg = String(format: format, arguments: args)
  FileHandle.standardError.write(Data("\"\(yyinname)\", line \(lineno): warning: \(msg)\n".utf8))
}

func error(_ format: String, _ args: CVarArg...) {
  let msg = String(format: format, arguments: args)
  FileHandle.standardError.write(Data("\"\(yyinname)\", line \(lineno): \(msg)\n".utf8))
  migErrors += 1
}
