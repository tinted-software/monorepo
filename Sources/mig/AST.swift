// AST.swift — Core data types for the Mach Interface Generator

import Foundation

// MARK: - String types

typealias string_t = String
typealias identifier_t = String

let strNULL = ""

// MARK: - Flags

struct IPCTypeFlags: OptionSet {
  let rawValue: UInt
  init(rawValue: UInt) { self.rawValue = rawValue }

  static let none = IPCTypeFlags([])
  static let physicalCopy = IPCTypeFlags(rawValue: 0x01)
  static let overwrite = IPCTypeFlags(rawValue: 0x02)
  static let dealloc = IPCTypeFlags(rawValue: 0x04)
  static let notDealloc = IPCTypeFlags(rawValue: 0x08)
  static let maybeDealloc = IPCTypeFlags(rawValue: 0x10)
  static let sameCount = IPCTypeFlags(rawValue: 0x20)
  static let countInOut = IPCTypeFlags(rawValue: 0x40)
  static let retCode = IPCTypeFlags(rawValue: 0x80)
  static let autoFlag = IPCTypeFlags(rawValue: 0x100)
  static let constFlag = IPCTypeFlags(rawValue: 0x200)
}

enum DeallocKind {
  case no  // do not deallocate
  case yes  // always deallocate
  case maybe  // deallocate according to parameter
}

// MARK: - IPC Type

class IPCType {
  var itName: identifier_t = ""  // MIG's name for this type
  var itNext: IPCType? = nil  // next type in symbol table

  var itTypeSize: UInt = 0  // size of the C type
  var itPadSize: UInt = 0  // amount of padding after data
  var itMinTypeSize: UInt = 0  // minimal amount of space occupied by data

  var itInName: UInt = 0  // name supplied to kernel in sent msg
  var itOutName: UInt = 0  // name in received msg
  var itSize: UInt = 0
  var itNumber: UInt = 1
  var itKPD_Number: UInt = 0  // number of Kernel Processed Data entries
  var itInLine: Bool = true
  var itMigInLine: Bool = false  // MIG presents data as InLine, although it is sent OOL
  var itPortType: Bool = false

  var itInNameStr: string_t = ""  // string form of itInName
  var itOutNameStr: string_t = ""  // string form of itOutName

  var itStruct: Bool = true
  var itString: Bool = false
  var itVarArray: Bool = false
  var itNoOptArray: Bool = false
  var itNative: Bool = false  // User specified a native (C) type
  var itNativePointer: Bool = false  // The user will pass a pointer to the native C type

  var itElement: IPCType? = nil

  var itUserType: identifier_t = ""
  var itServerType: identifier_t = ""
  var itTransType: identifier_t = ""

  var itKPDType: identifier_t = ""  // descriptors for KPD type of arguments

  var itInTrans: identifier_t? = nil  // may be nil
  var itOutTrans: identifier_t? = nil  // may be nil
  var itDestructor: identifier_t? = nil  // may be nil
  var itBadValue: identifier_t? = nil  // Excluded value for PointerToIfNot
  var itOOL_Number: UInt = 0

  // Helpers
  var isKernelProcData: Bool { !itInLine || itPortType }
  var isMultipleKPD: Bool { itKPD_Number > 1 }
  var isMigInlineEmul: Bool { itMigInLine }
  var isVariableSizedUntyped: Bool { itVarArray && itInLine && !itPortType }
  var isOptionalNative: Bool { itNative && itNativePointer && itBadValue != nil }
}

nonisolated(unsafe) let itNULL: IPCType? = nil

// MARK: - IPC built-in type names

let MACH_MSG_TYPE_UNSTRUCTURED: UInt = 0
let MACH_MSG_TYPE_BIT: UInt = 0
let MACH_MSG_TYPE_BOOLEAN: UInt = 0
let MACH_MSG_TYPE_INTEGER_8: UInt = 9
let MACH_MSG_TYPE_INTEGER_16: UInt = 1
let MACH_MSG_TYPE_INTEGER_32: UInt = 2
let MACH_MSG_TYPE_INTEGER_64: UInt = 3
let MACH_MSG_TYPE_CHAR: UInt = 8
let MACH_MSG_TYPE_BYTE: UInt = 9
let MACH_MSG_TYPE_REAL_32: UInt = 10
let MACH_MSG_TYPE_REAL_64: UInt = 11
let MACH_MSG_TYPE_STRING: UInt = 12
let MACH_MSG_TYPE_STRING_C: UInt = 12

let MACH_MSG_TYPE_MOVE_RECEIVE: UInt = 16
let MACH_MSG_TYPE_COPY_SEND: UInt = 19
let MACH_MSG_TYPE_MAKE_SEND: UInt = 20
let MACH_MSG_TYPE_MOVE_SEND: UInt = 17
let MACH_MSG_TYPE_MAKE_SEND_ONCE: UInt = 21
let MACH_MSG_TYPE_MOVE_SEND_ONCE: UInt = 18
let MACH_MSG_TYPE_PORT_NAME: UInt = 15
let MACH_MSG_TYPE_PORT_RECEIVE: UInt = 17
let MACH_MSG_TYPE_PORT_SEND: UInt = 17
let MACH_MSG_TYPE_PORT_SEND_ONCE: UInt = 18
let MACH_MSG_TYPE_POLYMORPHIC: UInt = uint_max

// MARK: - ConsumeOnSendError

let ConsumeOnSendErrorNone: UInt = 0
let ConsumeOnSendErrorTimeout: UInt = 1
let ConsumeOnSendErrorAny: UInt = 2

// MARK: - Argument kinds

// base kind values
let akeNone: UInt = 0
let akeNormal: UInt = 1
let akeRequestPort: UInt = 2
let akeWaitTime: UInt = 3
let akeReplyPort: UInt = 4
let akeMsgOption: UInt = 5
let akeMsgSeqno: UInt = 6
let akeRetCode: UInt = 7
let akeNdrCode: UInt = 8
let akeCount: UInt = 9
let akePoly: UInt = 10
let akeDealloc: UInt = 11
let akeCountInOut: UInt = 12
let akeSameCount: UInt = 13
let akeSubCount: UInt = 14
let akeImplicit: UInt = 15
let akeSecToken: UInt = 16
let akeAuditToken: UInt = 17
let akeContextToken: UInt = 18
let akeSendTime: UInt = 19

let akeBITS: UInt = 0x0000_003f

// bit flags
let akbRequest: UInt = 0x0000_0040  // has a msg_type in request
let akbReply: UInt = 0x0000_0080  // has a msg_type in reply
let akbUserArg: UInt = 0x0000_0100  // an arg on user-side
let akbServerArg: UInt = 0x0000_0200  // an arg on server-side
let akbSend: UInt = 0x0000_0400  // value carried in request
let akbSendBody: UInt = 0x0000_0800  // value carried in request body
let akbSendSnd: UInt = 0x0000_1000  // value stuffed into request
let akbSendRcv: UInt = 0x0000_2000  // value grabbed from request
let akbReturn: UInt = 0x0000_4000  // value carried in reply
let akbReturnBody: UInt = 0x0000_8000  // value carried in reply body
let akbReturnSnd: UInt = 0x0001_0000  // value stuffed into reply
let akbReturnRcv: UInt = 0x0002_0000  // value grabbed from reply
let akbReturnNdr: UInt = 0x0004_0000  // needs NDR conversion in reply
let akbReplyInit: UInt = 0x0008_0000  // reply value doesn't come from target routine
let akbReplyCopy: UInt = 0x0020_0000  // copy reply value from request
let akbVarNeeded: UInt = 0x0040_0000  // may need local var in server
let akbDestroy: UInt = 0x0080_0000  // call destructor function
let akbVariable: UInt = 0x0100_0000  // variable size inline data
let akbSendNdr: UInt = 0x0400_0000  // needs NDR conversion in request
let akbSendKPD: UInt = 0x0800_0000  // arg in KPD of Request
let akbReturnKPD: UInt = 0x1000_0000  // arg in KPD of Reply
let akbUserImplicit: UInt = 0x2000_0000  // arg is Impl on user side
let akbServerImplicit: UInt = 0x4000_0000  // arg is Impl on server side
let akbOverwrite: UInt = 0x8000_0000

let akbSendBits = akbSend | akbSendBody | akbSendSnd | akbSendRcv
let akbReturnBits = akbReturn | akbReturnBody | akbReturnSnd | akbReturnRcv
let akbSendReturnBits = akbSendBits | akbReturnBits

// Combined argument kinds
let akNone = akeNone

let akIn = akeNormal | akbUserArg | akbServerArg | akbRequest | akbSendBits
let akOut = akeNormal | akbUserArg | akbServerArg | akbReply | akbReturnBits
let akServerImpl = akeImplicit | akbServerArg | akbServerImplicit | akbSend | akbSendRcv
let akUserImpl = akeImplicit | akbUserArg | akbUserImplicit | akbReturn | akbReturnRcv
let akServerSecToken = akeSecToken | akbServerArg | akbServerImplicit | akbSend | akbSendRcv
let akUserSecToken = akeSecToken | akbUserArg | akbUserImplicit | akbReturn | akbReturnRcv
let akSecToken =
  akeSecToken | akbServerArg | akbServerImplicit | akbSend | akbSendRcv | akbUserArg
  | akbUserImplicit | akbReturn | akbReturnRcv
let akServerAuditToken = akeAuditToken | akbServerArg | akbServerImplicit | akbSend | akbSendRcv
let akUserAuditToken = akeAuditToken | akbUserArg | akbUserImplicit | akbReturn | akbReturnRcv
let akAuditToken =
  akeAuditToken | akbServerArg | akbServerImplicit | akbSend | akbSendRcv | akbUserArg
  | akbUserImplicit | akbReturn | akbReturnRcv
let akServerContextToken = akeContextToken | akbServerArg | akbServerImplicit | akbSend | akbSendRcv
let akMsgSeqno = akeMsgSeqno | akbServerArg | akbServerImplicit | akbSend | akbSendRcv
let akInOut =
  akeNormal | akbUserArg | akbServerArg | akbRequest | akbReply | akbSendBits | akbReturnBits
  | akbReplyCopy
let akRequestPort = akeRequestPort | akbUserArg | akbServerArg | akbSend | akbSendSnd | akbSendRcv
let akWaitTime = akeWaitTime | akbUserArg
let akSendTime = akeSendTime | akbUserArg
let akMsgOption = akeMsgOption | akbUserArg
let akReplyPort = akeReplyPort | akbUserArg | akbServerArg | akbSend | akbSendSnd | akbSendRcv
let akUReplyPort = akeReplyPort | akbUserArg | akbSend | akbSendSnd | akbSendRcv
let akSReplyPort = akeReplyPort | akbServerArg | akbSend | akbSendSnd | akbSendRcv
let akRetCode = akeRetCode | akbReply | akbReturnBody
let akCount = akeCount | akbUserArg | akbServerArg
let akCountInOut = akeCountInOut | akbRequest | akbSendBits
let akDealloc = akeDealloc | akbUserArg
let akPoly = akePoly

// Helper functions for arg_kind_t
func akCheck(_ ak: UInt, _ bits: UInt) -> Bool { (ak & bits) != 0 }
func akCheckAll(_ ak: UInt, _ bits: UInt) -> Bool { akCheck(ak, bits) && (ak & bits) == bits }
func akAddFeature(_ ak: UInt, _ bits: UInt) -> UInt { ak | bits }
func akRemFeature(_ ak: UInt, _ bits: UInt) -> UInt { ak & ~bits }
func akIdent(_ ak: UInt) -> UInt { ak & akeBITS }

func argIsIn(_ arg: Argument) -> Bool {
  akIdent(arg.argKind) == akeNormal && akCheck(arg.argKind, akbRequest)
}
func argIsOut(_ arg: Argument) -> Bool {
  akIdent(arg.argKind) == akeNormal && akCheck(arg.argKind, akbReply)
}

// MARK: - Argument

class Argument {
  var argName: identifier_t = ""
  var argNext: Argument? = nil

  var argKind: UInt = akNone
  var argType: IPCType? = nil

  var argVarName: string_t = ""  // local variable and argument names
  var argMsgField: string_t = ""  // message field's name
  var argTTName: string_t = ""  // name for msg_type fields, static vars
  var argPadName: string_t = ""  // name for pad field in msg
  var argSuffix: string_t = ""  // name extension for KPDs

  var argFlags: IPCTypeFlags = .none
  var argDeallocate: DeallocKind = .no
  var argCountInOut: Bool = false

  var argRoutine: Routine? = nil

  var argCount: Argument? = nil  // our count arg, if present
  var argSubCount: Argument? = nil  // our sub-count arg
  var argCInOut: Argument? = nil  // our CountInOut arg
  var argPoly: Argument? = nil  // our poly arg
  var argDealloc: Argument? = nil  // our dealloc arg
  var argSameCount: Argument? = nil  // the arg to take the count from
  var argParent: Argument? = nil  // in a count or poly arg, the base arg
  var argMultiplier: UInt = 1  // multiplier for Count arguments

  var argRequestPos: UInt = 0
  var argReplyPos: UInt = 0
  var argByReferenceUser: Bool = false
  var argByReferenceServer: Bool = false
  var argTempOnStack: Bool = false

  var argInSegment: string_t = ""
  var argOutSegment: string_t = ""
}

// Helpers for arguments
extension Argument {
  var RPCPort: Bool { false }  // Will be set during type checking
  var RPCUserStruct: Bool { (argType?.itStruct ?? false) && (argType?.itInLine ?? false) }
  var RPCOutStruct: Bool {
    (argType?.itStruct ?? false) && argIsOut(self) && !(argType?.itVarArray ?? false)
  }
  var RPCOutWord: Bool {
    RPCUserStruct && (argType?.itSize ?? 0) <= 32 && (argType?.itNumber ?? 0) == 1 && argIsOut(self)
  }
  var RPCVariableArray: Bool { (argType?.itVarArray ?? false) }
  var RPCFixedArray: Bool {
    guard let t = argType else { return false }
    return (!t.itVarArray && !t.isMultipleKPD && t.itNumber > 1 && !RPCUserStruct) || t.itString
      || (RPCOutStruct && t.itSize <= 32 && t.itNumber == 1) || RPCOutStruct
  }
}

// MARK: - Routine

enum RoutineKind {
  case routine
  case simpleRoutine
}

class Routine {
  var rtName: identifier_t = ""
  var rtKind: RoutineKind = .routine
  var rtArgs: Argument? = nil
  var rtNumber: UInt = 0

  var rtUserName: identifier_t = ""
  var rtServerName: identifier_t = ""

  var rtOneWay: Bool { rtKind == .simpleRoutine }

  var rtSimpleRequest: Bool = false
  var rtSimpleReply: Bool = false
  var rtUseSpecialReplyPort: Bool = false
  var rtConsumeOnSendError: UInt = ConsumeOnSendErrorNone

  var rtNumRequestVar: UInt = 0
  var rtNumReplyVar: UInt = 0
  var rtMaxRequestPos: UInt = 0
  var rtMaxReplyPos: UInt = 0
  var rtRequestKPDs: UInt = 0
  var rtReplyKPDs: UInt = 0
  var rtOverwrite: UInt = 0
  var rtOverwriteKPDs: UInt = 0

  var rtNoReplyArgs: Bool = false

  var rtRequestFits: Bool = true
  var rtReplyFits: Bool = true
  var rtRequestUsedLimit: Bool = false
  var rtReplyUsedLimit: Bool = false
  var rtRequestSizeKnown: UInt = 0
  var rtReplySizeKnown: UInt = 0

  var rtServerImpl: UInt = 0
  var rtUserImpl: UInt = 0

  var rtRetCArg: Argument? = nil
  var rtRequestPort: Argument? = nil
  var rtReplyPort: Argument? = nil
  var rtRetCode: Argument? = nil
  var rtNdrCode: Argument? = nil
  var rtWaitTime: Argument? = nil
  var rtMsgOption: Argument? = nil

  var rtCountPortsIn: UInt = 0
  var rtCountOolPortsIn: UInt = 0
  var rtCountOolIn: UInt = 0
  var rtCountPortsOut: UInt = 0
  var rtCountOolPortsOut: UInt = 0
  var rtCountOolOut: UInt = 0
  var rtTempBytesOnStack: UInt = 0
}

func rtMessOnStack(_ rt: Routine) -> Bool { rt.rtRequestFits && rt.rtReplyFits }

// MARK: - Statement

enum StatementKind {
  case routine
  case import_
  case uImport
  case sImport
  case dImport
  case iImport
  case rcsDecl
}

func importName(_ sk: StatementKind) -> String {
  switch sk {
  case .import_: return "Import"
  case .sImport: return "SImport"
  case .uImport: return "UImport"
  case .iImport: return "IImport"
  case .dImport: return "DImport"
  default: fatal("import_name: not import statement")
  }
}

class Statement {
  var stKind: StatementKind = .routine
  var stNext: Statement? = nil
  var stRoutine: Routine? = nil
  var stFileName: string_t = ""
}

// MARK: - Machine-specific

let NBBY: UInt = 8
let PortSize: UInt = UInt(MemoryLayout<UInt32>.size) * NBBY  // mach_port_t is 32-bit
let itWordAlign: UInt = UInt(MemoryLayout<Int>.stride)
let uint_max: UInt = UInt.max

nonisolated(unsafe) var machine_integer_name: string_t = "MACH_MSG_TYPE_INTEGER_32"
nonisolated(unsafe) var machine_integer_size: UInt = MACH_MSG_TYPE_INTEGER_32
nonisolated(unsafe) var machine_integer_bits: UInt = 32

func machine_padding(_ bytes: UInt) -> UInt {
  return (bytes & 3) != 0 ? (4 - (bytes & 3)) : 0
}
