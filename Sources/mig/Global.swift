// Global.swift — Global settings for the Mach Interface Generator

import Foundation

struct Global {
  nonisolated(unsafe) static var printVersion = false
  nonisolated(unsafe) static var beQuiet = false
  nonisolated(unsafe) static var beVerbose = false
  nonisolated(unsafe) static var beDebug = false
  nonisolated(unsafe) static var useMsgRPC = true
  nonisolated(unsafe) static var genSymTab = false
  nonisolated(unsafe) static var useEventLogger = false
  nonisolated(unsafe) static var beLint = false
  nonisolated(unsafe) static var beAnsiC = true
  nonisolated(unsafe) static var checkNDR = false
  nonisolated(unsafe) static var packMsg = true
  nonisolated(unsafe) static var useSplitHeaders = false
  nonisolated(unsafe) static var shortCircuit = false
  nonisolated(unsafe) static var useRPCTrap = false
  nonisolated(unsafe) static var testRPCTrap = false
  nonisolated(unsafe) static var isVoucherCodeAllowed = true
  nonisolated(unsafe) static var emitCountAnnotations = false

  nonisolated(unsafe) static var isKernelUser = false
  nonisolated(unsafe) static var isKernelServer = false
  nonisolated(unsafe) static var useMachMsg2 = false
  nonisolated(unsafe) static var useSpecialReplyPort = false
  nonisolated(unsafe) static var hasUseSpecialReplyPort = false
  nonisolated(unsafe) static var hasConsumeOnSendError = false
  nonisolated(unsafe) static var consumeOnSendError: UInt = 0
  nonisolated(unsafe) static var maxServerDescrs = -1
  nonisolated(unsafe) static var maxServerReplyDescrs = -1

  nonisolated(unsafe) static var rcsId: string_t = ""
  nonisolated(unsafe) static var subsystemName: string_t = ""
  nonisolated(unsafe) static var subsystemBase: UInt = 0

  nonisolated(unsafe) static var msgOption: string_t = ""
  nonisolated(unsafe) static var waitTime: string_t = ""
  nonisolated(unsafe) static var sendTime: string_t = ""
  nonisolated(unsafe) static var errorProc = "MsgError"
  nonisolated(unsafe) static var serverPrefix = ""
  nonisolated(unsafe) static var userPrefix = ""
  nonisolated(unsafe) static var serverDemux: string_t = ""
  nonisolated(unsafe) static var serverImpl: string_t = ""
  nonisolated(unsafe) static var serverSubsys: string_t = ""
  nonisolated(unsafe) static var maxMessSizeOnStack = -1
  nonisolated(unsafe) static var userTypeLimit = -1

  nonisolated(unsafe) static var userFilePrefix: string_t = ""
  nonisolated(unsafe) static var userHeaderFileName: string_t = ""
  nonisolated(unsafe) static var serverHeaderFileName: string_t = ""
  nonisolated(unsafe) static var internalHeaderFileName: string_t = ""
  nonisolated(unsafe) static var definesHeaderFileName: string_t = ""
  nonisolated(unsafe) static var userFileName: string_t = ""
  nonisolated(unsafe) static var serverFileName: string_t = ""
  /// Output path for a types-only header (no subsystem required)
  nonisolated(unsafe) static var typesHeaderFileName: string_t = ""
  /// True when we are processing a types-only .defs file (no subsystem block)
  nonisolated(unsafe) static var typesOnlyMode: Bool = false

  nonisolated(unsafe) static var generationDate: string_t = ""

  static let newCDecl = "(defined(__STDC__) || defined(c_plusplus))"
  static let lintLib = "defined(LINTLIBRARY)"

  static let MIG_VERSION = "mig (Swift) 1.0"

  static func initGlobal() {
    yyinname = "<no name yet>"
  }

  static func moreGlobal() {
    if subsystemName.isEmpty {
      if typesOnlyMode {
        // Types-only mode: no subsystem required — skip all subsystem-derived defaults.
        return
      }
      fatal("no SubSystem declaration")
    }

    if userHeaderFileName.isEmpty {
      userHeaderFileName = subsystemName + ".h"
    } else if userHeaderFileName == "/dev/null" {
      userHeaderFileName = ""
    }

    if userFileName.isEmpty {
      userFileName = subsystemName + "User.c"
    } else if userFileName == "/dev/null" {
      userFileName = ""
    }

    if serverFileName.isEmpty {
      serverFileName = subsystemName + "Server.c"
    } else if serverFileName == "/dev/null" {
      serverFileName = ""
    }

    if serverDemux.isEmpty {
      serverDemux = subsystemName + "_server"
    }

    if serverImpl.isEmpty {
      serverImpl = subsystemName + "_impl"
    }

    if serverSubsys.isEmpty {
      serverSubsys = (serverPrefix.isEmpty ? subsystemName : serverPrefix + subsystemName)
      serverSubsys = serverSubsys + "_subsystem"
    }

    if hasUseSpecialReplyPort && !beAnsiC {
      fatal("Cannot use UseSpecialReplyPort in non ANSI mode")
    }

    if useMachMsg2 {
      if !beAnsiC || useRPCTrap || checkNDR {
        fatal("KernelServer does not support the given options.")
      }
    }
  }
}

// Alias for code readability
typealias G = Global
