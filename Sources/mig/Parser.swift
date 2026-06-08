// Parser.swift — Recursive descent parser for MIG .defs files

import Foundation

// MARK: - Token stream

class TokenStream {
  private let lexer: Lexer
  private var buffer: [Token] = []
  private var pos = 0

  init(lexer: Lexer) {
    self.lexer = lexer
  }

  private func loadMore() {
    if pos >= buffer.count {
      buffer.append(lexer.nextToken())
    }
  }

  var current: Token {
    loadMore()
    return pos < buffer.count ? buffer[pos] : .eof
  }
  func peek() -> Token { current }

  func advance() -> Token {
    loadMore()
    let tok = pos < buffer.count ? buffer[pos] : .eof
    if pos < buffer.count { pos += 1 }
    return tok
  }

  func match(_ expected: Token) -> Bool {
    if current == expected {
      _ = advance()
      return true
    }
    return false
  }

  func expect(_ expected: Token) {
    if !match(expected) {
      error("syntax error: expected token")
    }
  }

  func isEOF() -> Bool { pos >= buffer.count && loadMoreCheck() }

  private func loadMoreCheck() -> Bool {
    let tok = lexer.nextToken()
    buffer.append(tok)
    return tok == .eof
  }

  func matchSemi() -> Bool { match(.semi) }

  // Lexer state methods
  func lookString() { lexer.lookString() }
  func lookFileName() { lexer.lookFileName() }
  func lookQString() { lexer.lookQString() }

  var identifierValue: String? {
    if case .identifier(let s) = current { return s }
    return nil
  }
  var numberValue: UInt? {
    if case .number(let n) = current { return n }
    return nil
  }
  var stringValue: String? {
    if case .string(let s) = current { return s }
    return nil
  }
  var qStringValue: String? {
    if case .qString(let s) = current { return s }
    return nil
  }
  var fileNameValue: String? {
    if case .fileName(let s) = current { return s }
    return nil
  }
  var symbolicTypeValue: (UInt, String, UInt, String, UInt)? {
    if case .symbolicType(let inn, let ins, let outn, let outs, let sz) = current {
      return (inn, ins, outn, outs, sz)
    }
    return nil
  }
  var ipcFlagValue: IPCTypeFlags? {
    if case .ipcFlag(let f) = current { return f }
    return nil
  }
}

// MARK: - Parser

class Parser {
  let tokens: TokenStream
  var statements: [Statement] = []

  init(tokens: TokenStream) {
    self.tokens = tokens
  }

  func parse() -> [Statement] {
    parseStatements()
    return statements
  }

  // MARK: - Statements

  private func parseStatements() {
    while !tokens.isEOF() {
      parseStatement()
    }
  }

  private func parseStatement() {
    switch tokens.current {
    case .subsystem:
      parseSubsystem()
      tokens.expect(.semi)

    case .waitTime:
      parseWaitTime()
      tokens.expect(.semi)

    case .sendTime:
      parseSendTime()
      tokens.expect(.semi)

    case .msgOption:
      parseMsgOption()
      tokens.expect(.semi)

    case .useSpecialReplyPort:
      parseUseSpecialReplyPort()
      tokens.expect(.semi)

    case .consumeOnSendError:
      parseConsumeOnSendError()
      tokens.expect(.semi)

    case .userTypeLimit:
      parseUserTypeLimit()
      tokens.expect(.semi)

    case .onStackLimit:
      parseOnStackLimit()
      tokens.expect(.semi)

    case .errorProc:
      parseErrorProc()
      tokens.expect(.semi)

    case .serverPrefix:
      parseServerPrefix()
      tokens.expect(.semi)

    case .userPrefix:
      parseUserPrefix()
      tokens.expect(.semi)

    case .serverDemux:
      parseServerDemux()
      tokens.expect(.semi)

    case .type_:
      parseTypeDecl()
      tokens.expect(.semi)

    case .routine, .simpleRoutine:
      parseRoutineDecl()
      tokens.expect(.semi)

    case .skip:
      parseSkip()
      tokens.expect(.semi)

    case .import_, .uImport, .sImport, .iImport, .dImport:
      parseImport()
      tokens.expect(.semi)

    case .rcsId:
      parseRCSDecl()
      tokens.expect(.semi)

    case .semi:
      _ = tokens.advance()

    default:
      error("syntax error at token")
      // Skip to next semicolon
      while !tokens.isEOF() && !tokens.match(.semi) {
        _ = tokens.advance()
      }
    }
  }

  // MARK: - Subsystem

  private func parseSubsystem() {
    tokens.expect(.subsystem)  // subsystem

    // Parse SubsystemMods
    while true {
      switch tokens.current {
      case .kernelUser:
        _ = tokens.advance()
        if Global.isKernelUser { warning("duplicate KernelUser keyword") }
        if !Global.useMsgRPC {
          warning("with KernelUser the -R option is meaningless")
          Global.useMsgRPC = true
        }
        Global.isKernelUser = true
      case .kernelServer:
        _ = tokens.advance()
        if Global.isKernelServer { warning("duplicate KernelServer keyword") }
        Global.isKernelServer = true
      default:
        break
      }
      // Break if we see something that's not a subsystem mod
      guard case .kernelUser = tokens.current else { break }
      guard case .kernelServer = tokens.current else { break }
      // Actually need to check carefully - break on identifier
      if case .identifier = tokens.current { break }
      if case .kernelUser = tokens.current { continue }
      if case .kernelServer = tokens.current { continue }
      break
    }

    // SubsystemName
    guard let name = tokens.identifierValue else {
      error("expected subsystem name")
      return
    }
    _ = tokens.advance()
    Global.subsystemName = name

    // SubsystemBase
    guard let base = tokens.numberValue else {
      error("expected subsystem base number")
      return
    }
    _ = tokens.advance()
    Global.subsystemBase = base

    if Global.beVerbose {
      let ku = Global.isKernelUser ? ", KernelUser" : ""
      let ks = Global.isKernelServer ? ", KernelServer" : ""
      print("Subsystem \(name): base = \(base)\(ku)\(ks)\n")
    }
  }

  // MARK: - Options

  private func parseWaitTime() {
    _ = tokens.advance()  // waitTime/syWaitTime
    tokens.lookString()
    if case .noWaitTime = tokens.current {
      _ = tokens.advance()
      Global.waitTime = ""
      if Global.beVerbose { print("NoWaitTime\n") }
    } else if let s = tokens.stringValue {
      _ = tokens.advance()
      Global.waitTime = s
      if Global.beVerbose { print("WaitTime \(s)\n") }
    } else {
      error("expected string after WaitTime")
    }
  }

  private func parseSendTime() {
    _ = tokens.advance()
    tokens.lookString()
    if case .noSendTime = tokens.current {
      _ = tokens.advance()
      Global.sendTime = ""
      if Global.beVerbose { print("NoSendTime\n") }
    } else if let s = tokens.stringValue {
      _ = tokens.advance()
      Global.sendTime = s
      if Global.beVerbose { print("SendTime \(s)\n") }
    } else {
      error("expected string after SendTime")
    }
  }

  private func parseMsgOption() {
    tokens.lookString()
    _ = tokens.advance()  // msgOption keyword
    if let s = tokens.stringValue {
      _ = tokens.advance()
      if s == "MACH_MSG_OPTION_NONE" {
        Global.msgOption = ""
        if Global.beVerbose { print("MsgOption: canceled\n") }
      } else {
        Global.msgOption = s
        if Global.beVerbose { print("MsgOption \(s)\n") }
      }
    }
  }

  private func parseUseSpecialReplyPort() {
    _ = tokens.advance()  // useSpecialReplyPort
    if let n = tokens.numberValue {
      _ = tokens.advance()
      Global.useSpecialReplyPort = (n != 0)
      Global.hasUseSpecialReplyPort = Global.hasUseSpecialReplyPort || Global.useSpecialReplyPort
    }
  }

  private func parseConsumeOnSendError() {
    tokens.lookString()
    _ = tokens.advance()  // consumeOnSendError
    if let s = tokens.stringValue {
      _ = tokens.advance()
      let lower = s.lowercased()
      if lower == "none" {
        Global.consumeOnSendError = ConsumeOnSendErrorNone
      } else if lower == "timeout" {
        Global.consumeOnSendError = ConsumeOnSendErrorTimeout
        Global.hasConsumeOnSendError = true
      } else if lower == "any" {
        Global.consumeOnSendError = ConsumeOnSendErrorAny
        Global.hasConsumeOnSendError = true
      } else {
        error("syntax error in ConsumeOnSendError")
      }
    }
  }

  private func parseUserTypeLimit() {
    _ = tokens.advance()
    if let n = tokens.numberValue {
      _ = tokens.advance()
      Global.userTypeLimit = Int(n)
    }
  }

  private func parseOnStackLimit() {
    _ = tokens.advance()
    if let n = tokens.numberValue {
      _ = tokens.advance()
      Global.maxMessSizeOnStack = Int(n)
    }
  }

  private func parseErrorProc() {
    _ = tokens.advance()
    if let name = tokens.identifierValue {
      _ = tokens.advance()
      Global.errorProc = name
      if Global.beVerbose { print("ErrorProc \(name)\n") }
    }
  }

  private func parseServerPrefix() {
    _ = tokens.advance()
    if let name = tokens.identifierValue {
      _ = tokens.advance()
      Global.serverPrefix = name
      if Global.beVerbose { print("ServerPrefix \(name)\n") }
    }
  }

  private func parseUserPrefix() {
    _ = tokens.advance()
    if let name = tokens.identifierValue {
      _ = tokens.advance()
      Global.userPrefix = name
      if Global.beVerbose { print("UserPrefix \(name)\n") }
    }
  }

  private func parseServerDemux() {
    _ = tokens.advance()
    if let name = tokens.identifierValue {
      _ = tokens.advance()
      Global.serverDemux = name
      if Global.beVerbose { print("ServerDemux \(name)\n") }
    }
  }

  // MARK: - Import

  private func parseImport() {
    let kind: StatementKind
    switch tokens.advance() {
    case .import_: kind = .import_
    case .uImport: kind = .uImport
    case .sImport: kind = .sImport
    case .iImport: kind = .iImport
    case .dImport: kind = .dImport
    default: kind = .import_
    }

    tokens.lookFileName()
    guard let fn = tokens.fileNameValue else {
      error("expected filename after import")
      return
    }
    _ = tokens.advance()

    let st = Statement()
    st.stKind = kind
    st.stFileName = fn
    statements.append(st)

    if Global.beVerbose {
      print("\(importName(kind)) \(fn)\n")
    }
  }

  // MARK: - RCSDecl

  private func parseRCSDecl() {
    _ = tokens.advance()  // rcsId keyword
    tokens.lookQString()
    guard let id = tokens.qStringValue else {
      error("expected string after RCSId")
      return
    }
    _ = tokens.advance()

    if !Global.rcsId.isEmpty {
      warning("previous RCS decl will be ignored")
    }
    Global.rcsId = id
    if Global.beVerbose { print("RCSId \(id)\n") }
  }

  // MARK: - Skip

  private func parseSkip() {
    _ = tokens.advance()
    // rtSkip is called - increment routine number
    // We'll track this via the routine counter
  }

  // MARK: - Type declarations

  private func parseTypeDecl() {
    _ = tokens.advance()  // type keyword
    let type = parseNamedTypeSpec()

    guard let name = type?.itName, !name.isEmpty else {
      error("expected type name")
      return
    }

    if let existing = TypeSystem.lookUp(name) {
      warning("overriding previous definition of \(name)")
    }
    TypeSystem.insert(name: name, type: type!)
  }

  // MARK: - Routine declarations

  private func parseRoutineDecl() {
    let tok = tokens.advance()
    guard let name = tokens.identifierValue else {
      error("expected routine name")
      return
    }
    _ = tokens.advance()

    let args = parseArguments()

    let rt: Routine
    if case .simpleRoutine = tok {
      rt = makeSimpleRoutine(name: name, args: args)
    } else {
      rt = makeRoutine(name: name, args: args)
    }

    checkRoutine(rt)

    let st = Statement()
    st.stKind = .routine
    st.stRoutine = rt
    statements.append(st)

    if Global.beVerbose {
      printRoutine(rt)
    }
  }

  // Track routine number
  private var routineNumber: UInt = 0

  private func makeRoutine(name: String, args: Argument?) -> Routine {
    let rt = Routine()
    routineNumber += 1
    rt.rtNumber = routineNumber
    rt.rtName = name
    rt.rtKind = .routine
    rt.rtArgs = args
    return rt
  }

  private func makeSimpleRoutine(name: String, args: Argument?) -> Routine {
    let rt = Routine()
    routineNumber += 1
    rt.rtNumber = routineNumber
    rt.rtName = name
    rt.rtKind = .simpleRoutine
    rt.rtArgs = args
    return rt
  }

  func skipRoutine() {
    routineNumber += 1
  }

  // MARK: - Argument parsing

  private func parseArguments() -> Argument? {
    guard tokens.match(.lParen) else {
      error("expected '('")
      return nil
    }

    if tokens.match(.rParen) {
      return nil
    }

    let firstArg = parseArgumentList()
    tokens.expect(.rParen)
    return firstArg
  }

  private func parseArgumentList() -> Argument? {
    let arg = parseArgumentOrTrailer()

    // Check for more arguments separated by semicolons
    while tokens.match(.semi) {
      // peek ahead - if we see a direction keyword, argument type, or identifier -> more args
      if isStartOfArgument() {
        let nextArg = parseArgumentOrTrailer()
        // Link: arg -> argNext = nextArg...append nextArg at end
        var last = arg
        while last?.argNext != nil {
          last = last?.argNext
        }
        last?.argNext = nextArg
      } else {
        break
      }
    }

    return arg
  }

  private func isStartOfArgument() -> Bool {
    switch tokens.current {
    case .in_, .out, .inOut, .requestPort, .replyPort, .sReplyPort, .uReplyPort,
      .waitTime, .sendTime, .msgOption, .secToken, .serverSecToken,
      .userSecToken, .auditToken, .serverAuditToken, .userAuditToken,
      .serverContextToken, .msgSeqno, .userImpl, .serverImpl,
      .identifier, .colon:
      return true
    default:
      return false
    }
  }

  private func parseArgumentOrTrailer() -> Argument? {
    let direction = parseDirection()
    guard let name = tokens.identifierValue else {
      if case .identifier = tokens.current { _ = tokens.advance() }
      error("expected argument name")
      return nil
    }
    _ = tokens.advance()

    let argType = parseArgumentType()
    let flags = parseIPCFlags()

    let arg = Argument()
    arg.argKind = direction
    arg.argName = name
    arg.argType = argType
    arg.argFlags = flags

    // Check native type constraints
    if let t = argType, t.itNative {
      let kind = akIdent(direction)
      if kind != akeImplicit
        && (akCheck(direction, akIn) || akCheck(direction, akOut) || akCheck(direction, akInOut))
      {
        // Valid
      } else if kind == akeImplicit {
        // Trailer, OK
      } else {
        error("Illegal direction specified for native type")
      }

      if !t.itNativePointer && !akCheck(direction, akIn) {
        error("ValueOf only valid for in")
      }

      if t.itBadValue != nil && !akCheck(direction, akIn) {
        error("PointerToIfNot only valid for in")
      }
    }

    return arg
  }

  private func parseDirection() -> UInt {
    switch tokens.current {
    case .in_:
      _ = tokens.advance()
      return akIn
    case .out:
      _ = tokens.advance()
      return akOut
    case .inOut:
      _ = tokens.advance()
      return akInOut
    case .requestPort:
      _ = tokens.advance()
      return akRequestPort
    case .replyPort:
      _ = tokens.advance()
      return akReplyPort
    case .sReplyPort:
      _ = tokens.advance()
      return akSReplyPort
    case .uReplyPort:
      _ = tokens.advance()
      return akUReplyPort
    case .waitTime:
      _ = tokens.advance()
      return akWaitTime
    case .sendTime:
      _ = tokens.advance()
      return akSendTime
    case .msgOption:
      _ = tokens.advance()
      return akMsgOption
    case .secToken:
      _ = tokens.advance()
      return akSecToken
    case .serverSecToken:
      _ = tokens.advance()
      return akServerSecToken
    case .userSecToken:
      _ = tokens.advance()
      return akUserSecToken
    case .auditToken:
      _ = tokens.advance()
      return akAuditToken
    case .serverAuditToken:
      _ = tokens.advance()
      return akServerAuditToken
    case .userAuditToken:
      _ = tokens.advance()
      return akUserAuditToken
    case .serverContextToken:
      _ = tokens.advance()
      return akServerContextToken
    case .msgSeqno:
      _ = tokens.advance()
      return akMsgSeqno
    case .userImpl:
      _ = tokens.advance()
      return akUserImpl
    case .serverImpl:
      _ = tokens.advance()
      return akServerImpl
    default: return akNone
    }
  }

  private func parseArgumentType() -> IPCType? {
    guard tokens.match(.colon) else {
      error("expected ':' before argument type")
      return nil
    }

    switch tokens.current {
    case .identifier:
      guard let name = tokens.identifierValue else { return nil }
      _ = tokens.advance()
      if let type = TypeSystem.lookUp(name) {
        return type
      } else {
        error("type '\(name)' not defined")
        return nil
      }

    case .pointerTo, .pointerToIfNot, .valueOf:
      return parseNativeTypeSpec()

    default:
      // NamedTypeSpec
      return parseNamedTypeSpec()
    }
  }

  private func parseIPCFlags() -> IPCTypeFlags {
    var flags: IPCTypeFlags = .none

    while tokens.match(.comma) {
      if case .ipcFlag(let f) = tokens.current {
        if flags.contains(f) {
          warning("redundant IPC flag ignored")
        } else {
          flags.insert(f)
        }
        _ = tokens.advance()
      }
      // Check for dealloc[]
      if case .ipcFlag(let f) = tokens.current, f == .dealloc {
        if tokens.current == .lBrack {  // peek next needs buffer
          // We need lookahead - this is a limitation of simple recursive descent
          // For now, treat dealloc[] as maybeDealloc at code gen time
        }
      }
    }

    return flags
  }

  // MARK: - Type spec parsing

  func parseNamedTypeSpec() -> IPCType? {
    guard let name = tokens.identifierValue else {
      error("expected identifier in named type spec")
      return nil
    }
    _ = tokens.advance()

    guard tokens.match(.equal) else {
      error("expected '=' in type declaration")
      return nil
    }

    guard let type = parseTransTypeSpec() else { return nil }
    type.itName = name
    TypeSystem.typeDecl(name: name, type: type)
    return type
  }

  func parseTransTypeSpec() -> IPCType? {
    guard var type = parseTypeSpec() else { return nil }
    type = TypeSystem.resetType(type)

    // Check for trans modifiers
    while true {
      switch tokens.current {
      case .inTran:
        _ = tokens.advance()
        guard tokens.match(.colon),
          let transType = tokens.identifierValue,
          let inTran = tokens.identifierValue
        else {
          error("expected inTran: type function(serverType)")
          return type
        }
        // Skip the function parameters part
        _ = tokens.advance()  // transType
        _ = tokens.advance()  // inTran name
        guard tokens.match(.lParen),
          let serverType = tokens.identifierValue,
          tokens.match(.rParen)
        else {
          error("expected inTran: type function(serverType)")
          return type
        }
        _ = tokens.advance()  // serverType
        if !type.itTransType.isEmpty && type.itTransType != transType {
          warning("conflicting translation types (\(type.itTransType), \(transType))")
        }
        type.itTransType = transType
        type.itInTrans = inTran
        type.itServerType = serverType

      case .outTran:
        _ = tokens.advance()
        guard tokens.match(.colon),
          let serverType = tokens.identifierValue,
          let outTran = tokens.identifierValue
        else {
          error("expected outTran: serverType function(transType)")
          return type
        }
        _ = tokens.advance()  // serverType
        _ = tokens.advance()  // outTran name
        guard tokens.match(.lParen),
          let transType = tokens.identifierValue,
          tokens.match(.rParen)
        else {
          error("expected outTran: serverType function(transType)")
          return type
        }
        _ = tokens.advance()  // transType
        if !type.itServerType.isEmpty && type.itServerType != serverType {
          warning("conflicting server types (\(type.itServerType), \(serverType))")
        }
        type.itServerType = serverType
        type.itOutTrans = outTran
        type.itTransType = transType

      case .destructor:
        _ = tokens.advance()
        guard tokens.match(.colon),
          let destructor = tokens.identifierValue
        else { return type }
        _ = tokens.advance()
        guard tokens.match(.lParen),
          let transType = tokens.identifierValue,
          tokens.match(.rParen)
        else { return type }
        _ = tokens.advance()
        type.itDestructor = destructor
        type.itTransType = transType

      case .cType:
        _ = tokens.advance()
        guard tokens.match(.colon),
          let cType = tokens.identifierValue
        else { return type }
        _ = tokens.advance()
        type.itUserType = cType
        type.itServerType = cType

      case .cUserType:
        _ = tokens.advance()
        guard tokens.match(.colon),
          let userType = tokens.identifierValue
        else { return type }
        _ = tokens.advance()
        type.itUserType = userType

      case .cServerType:
        _ = tokens.advance()
        guard tokens.match(.colon),
          let serverType = tokens.identifierValue
        else { return type }
        _ = tokens.advance()
        type.itServerType = serverType

      default:
        return type
      }
    }
  }

  func parseTypeSpec() -> IPCType? {
    switch tokens.current {
    case .array:
      return parseArrayType()
    case .caret:
      _ = tokens.advance()
      if let inner = parseTypeSpec() {
        return TypeSystem.ptrDecl(inner)
      }
      return nil
    case .struct_:
      return parseStructType()
    case .cString:
      return parseCStringType()
    case .pointerTo, .pointerToIfNot, .valueOf:
      return parseNativeTypeSpec()
    case .identifier:
      return parseBasicOrPrevType()
    case .number, .symbolicType(_, _, _, _, _):
      return parsePrimIPCType()
    default:
      // Try identifier first, then IPC type
      if let name = tokens.identifierValue {
        return parseBasicOrPrevType()
      }
      error("expected type spec")
      return nil
    }
  }

  func parseBasicOrPrevType() -> IPCType? {
    guard let name = tokens.identifierValue else { return nil }
    _ = tokens.advance()

    // Check if it's a previously defined type
    if TypeSystem.lookUp(name) != nil {
      return TypeSystem.prevDecl(name)
    }

    return nil
  }

  func parsePrimIPCType() -> IPCType? {
    // Try number
    if let n = tokens.numberValue {
      _ = tokens.advance()
      let type = TypeSystem.shortDecl(
        inname: n, instr: String(n), outname: n, outstr: String(n), dfault: 1)
      return type
    }

    // Try symbolic type
    if let (inn, ins, outn, outs, sz) = tokens.symbolicTypeValue {
      _ = tokens.advance()

      // Check for bar (alternate)
      if tokens.match(.bar) {
        let type2: IPCType
        if let n = tokens.numberValue {
          _ = tokens.advance()
          type2 = TypeSystem.shortDecl(
            inname: n, instr: String(n), outname: n, outstr: String(n), dfault: 1)
        } else if let (inn2, ins2, outn2, outs2, sz2) = tokens.symbolicTypeValue {
          _ = tokens.advance()
          type2 = TypeSystem.shortDecl(
            inname: inn2, instr: ins2, outname: outn2, outstr: outs2, dfault: 1)
        } else {
          return nil
        }

        // Merge: innumber = first, outnumber = second
        let mergedSize: UInt
        if sz == 0 {
          mergedSize = type2.itSize
        } else if type2.itSize == 0 {
          mergedSize = sz
        } else {
          if sz != type2.itSize {
            error("sizes in IPCTypes (\(sz), \(type2.itSize)) aren't equal")
          }
          mergedSize = sz
        }

        let merged = TypeSystem.shortDecl(
          inname: inn, instr: ins, outname: type2.itOutName, outstr: type2.itOutNameStr,
          dfault: mergedSize)
        return merged
      }

      let type = TypeSystem.shortDecl(
        inname: inn, instr: ins, outname: outn, outstr: outs, dfault: sz)
      return type
    }

    return nil
  }

  func parseArrayType() -> IPCType? {
    _ = tokens.advance()  // array

    // Check for various array forms
    if tokens.match(.lBrack) {
      if tokens.match(.rBrack) {
        // array[] of
        tokens.expect(.of)
        guard let inner = parseTypeSpec() else { return nil }
        return TypeSystem.varArrayDecl(number: 0, old: inner)
      } else if tokens.match(.star) {
        if tokens.match(.rBrack) {
          // array[*] of
          tokens.expect(.of)
          guard let inner = parseTypeSpec() else { return nil }
          return TypeSystem.varArrayDecl(number: 0, old: inner)
        } else if tokens.match(.colon) {
          // array[*:expr] of
          guard let n = parseInt() else { return nil }
          tokens.expect(.rBrack)
          tokens.expect(.of)
          guard let inner = parseTypeSpec() else { return nil }
          return TypeSystem.varArrayDecl(number: n, old: inner)
        }
      } else {
        // array[expr] of
        guard let n = parseInt() else { return nil }
        tokens.expect(.rBrack)
        tokens.expect(.of)
        guard let inner = parseTypeSpec() else { return nil }
        return TypeSystem.arrayDecl(number: n, old: inner)
      }
    }

    error("expected '[' after array")
    return nil
  }

  func parseStructType() -> IPCType? {
    _ = tokens.advance()  // struct
    tokens.expect(.lBrack)
    guard let n = parseInt() else { return nil }
    tokens.expect(.rBrack)
    tokens.expect(.of)
    guard let inner = parseTypeSpec() else { return nil }
    return TypeSystem.structDecl(number: n, old: inner)
  }

  func parseCStringType() -> IPCType? {
    _ = tokens.advance()  // c_string
    tokens.expect(.lBrack)

    let varying: Bool
    if tokens.match(.star) {
      varying = true
      tokens.expect(.colon)
    } else {
      varying = false
    }

    guard let n = parseInt() else { return nil }
    tokens.expect(.rBrack)
    return TypeSystem.cStringDecl(count: n, varying: varying)
  }

  func parseNativeTypeSpec() -> IPCType? {
    switch tokens.advance() {
    case .pointerTo:
      tokens.expect(.lParen)
      let typePhrase = parseTypePhrase()
      tokens.expect(.rParen)
      return TypeSystem.nativeType(typePhrase, pointer: true, badVal: nil)

    case .pointerToIfNot:
      tokens.expect(.lParen)
      let typePhrase = parseTypePhrase()
      tokens.expect(.comma)
      let badPhrase = parseTypePhrase()
      tokens.expect(.rParen)
      return TypeSystem.nativeType(typePhrase, pointer: true, badVal: badPhrase)

    case .valueOf:
      tokens.expect(.lParen)
      let typePhrase = parseTypePhrase()
      tokens.expect(.rParen)
      return TypeSystem.nativeType(typePhrase, pointer: false, badVal: nil)

    default:
      return nil
    }
  }

  func parseTypePhrase() -> String {
    var phrase = ""
    while let name = tokens.identifierValue {
      _ = tokens.advance()
      if phrase.isEmpty {
        phrase = name
      } else {
        phrase = phrase + " " + name
      }
    }
    return phrase
  }

  // MARK: - Integer expressions

  func parseInt() -> UInt? {
    return parseIntExp()
  }

  func parseIntExp() -> UInt? {
    guard var left = parseIntTerm() else { return nil }

    while true {
      if tokens.match(.plus) {
        guard let right = parseIntTerm() else { return nil }
        left = left + right
      } else if tokens.match(.minus) {
        guard let right = parseIntTerm() else { return nil }
        left = left - right
      } else {
        break
      }
    }

    return left
  }

  func parseIntTerm() -> UInt? {
    guard var left = parseIntFactor() else { return nil }

    while tokens.match(.star) {
      guard let right = parseIntFactor() else { return nil }
      left = left * right
    }
    while tokens.match(.div) {
      guard let right = parseIntFactor(), right != 0 else { return nil }
      left = left / right
    }

    return left
  }

  func parseIntFactor() -> UInt? {
    if let n = tokens.numberValue {
      _ = tokens.advance()
      return n
    }
    if tokens.match(.lParen) {
      guard let n = parseIntExp() else { return nil }
      tokens.expect(.rParen)
      return n
    }
    return nil
  }

  // MARK: - Stub implementations

  private func printRoutine(_ rt: Routine) {
    print("Routine \(rt.rtName) (#\(rt.rtNumber))\n")
    var arg = rt.rtArgs
    while let a = arg {
      if akCheck(a.argKind, akbUserArg | akbServerArg),
        ![akeCount, akeDealloc, akeNdrCode, akePoly].contains(akIdent(a.argKind))
      {
        let kindStr: String
        switch akIdent(a.argKind) {
        case akeRequestPort: kindStr = "RequestPort"
        case akeReplyPort: kindStr = "ReplyPort"
        case akeWaitTime: kindStr = "WaitTime"
        case akeSendTime: kindStr = "SendTime"
        case akeMsgOption: kindStr = "MsgOption"
        case akeMsgSeqno: kindStr = "MsgSeqno"
        case akeSecToken: kindStr = "SecToken"
        case akeAuditToken: kindStr = "AuditToken"
        case akeContextToken: kindStr = "ContextToken"
        default:
          let dirStr: String
          if akCheck(a.argKind, akIn) && akCheck(a.argKind, akOut) {
            dirStr = "InOut"
          } else if akCheck(a.argKind, akIn) {
            dirStr = "In"
          } else if akCheck(a.argKind, akOut) {
            dirStr = "Out"
          } else {
            dirStr = ""
          }
          kindStr = "\(dirStr) \(a.argName ?? "?") : \(a.argType?.itName ?? "?")"
        }
        print("\t\(kindStr)")
      }
      arg = a.argNext
    }
    print("")
  }

  private func checkRoutine(_ rt: Routine) {
    rt.rtSimpleRequest = true
    rt.rtSimpleReply = true

    // Create distinguished arguments
    let retCodeArg = Argument()
    retCodeArg.argKind = akRetCode
    retCodeArg.argName = "RetCode"
    retCodeArg.argVarName = "RetCode"
    retCodeArg.argMsgField = "RetCode"
    retCodeArg.argType = TypeSystem.itRetCodeType
    retCodeArg.argRoutine = rt
    rt.rtRetCode = retCodeArg

    // NDR code arg
    let ndrArg = Argument()
    ndrArg.argKind =
      akeNdrCode | akbRequest | akbReply | akbSendBody | akbSendSnd | akbReturnBody | akbReplyInit
    ndrArg.argName = "NDR"
    ndrArg.argVarName = "NDR_record"
    ndrArg.argMsgField = "NDR"
    ndrArg.argType = TypeSystem.itNdrCodeType ?? TypeSystem.itRetCodeType
    ndrArg.argRoutine = rt
    rt.rtNdrCode = ndrArg
    rt.rtSimpleRequest = false
    rt.rtSimpleReply = false

    // Walk through args, set names and positions
    var arg = rt.rtArgs
    var reqPos: UInt = 0
    var repPos: UInt = 0
    var reqKPDs: UInt = 0
    var repKPDs: UInt = 0
    var hasReplyArgs = false
    var hasRequestPort = false
    var hasReplyPort = false

    while let a = arg {
      // Set argVarName and argMsgField from argName if not already set
      if a.argVarName.isEmpty { a.argVarName = a.argName }
      if a.argMsgField.isEmpty { a.argMsgField = a.argName }
      if a.argPadName.isEmpty { a.argPadName = "_pad_\(a.argName)" }
      a.argRoutine = rt

      // Set argByReference based on type
      if let t = a.argType {
        if !t.itInLine || t.itVarArray || t.itNativePointer {
          if akCheck(a.argKind, akbUserArg) { a.argByReferenceUser = true }
          if akCheck(a.argKind, akbServerArg) { a.argByReferenceServer = true }
        }
      }

      // Track distinguished args
      let kind = akIdent(a.argKind)
      if kind == akeRequestPort {
        rt.rtRequestPort = a
        hasRequestPort = true
      }
      if kind == akeReplyPort {
        rt.rtReplyPort = a
        hasReplyPort = true
      }
      if kind == akeWaitTime { rt.rtWaitTime = a }
      if kind == akeMsgOption { rt.rtMsgOption = a }

      guard let ty = a.argType else {
        arg = a.argNext
        continue
      }

      // Set KPD type names for port args
      if ty.itPortType && !ty.itInLine {
        ty.itKPDType =
          ty.isMultipleKPD ? "mach_msg_ool_ports_descriptor_t" : "mach_msg_port_descriptor_t"
      } else if !ty.itInLine {
        ty.itKPDType = "mach_msg_ool_descriptor_t"
      }

      if akCheck(a.argKind, akbSendKPD) {
        reqPos += 1
        reqKPDs += ty.itKPD_Number
        rt.rtSimpleRequest = false
      }
      if akCheck(a.argKind, akbReturnKPD) {
        repPos += 1
        repKPDs += ty.itKPD_Number
        rt.rtSimpleReply = false
      }

      if akCheck(a.argKind, akbRequest) && akCheck(a.argKind, akbSendBody) {
        a.argRequestPos = reqPos
        reqPos += 1
        if ty.itVarArray { rt.rtSimpleRequest = false }
      }
      if akCheck(a.argKind, akbReply) && akCheck(a.argKind, akbReturnBody) {
        a.argReplyPos = repPos
        repPos += 1
        if ty.itVarArray { rt.rtSimpleReply = false }
        hasReplyArgs = true
      }

      arg = a.argNext
    }

    // Create request port if none found
    if !hasRequestPort {
      let reqPort = Argument()
      reqPort.argKind = akRequestPort
      reqPort.argName = "RequestPort"
      reqPort.argVarName = "RequestPort"
      reqPort.argMsgField = "RequestPort"
      reqPort.argType = TypeSystem.itRequestPortType
      reqPort.argRoutine = rt
      rt.rtRequestPort = reqPort
    }

    // Create reply port if none found
    if !hasReplyPort {
      let repPort = Argument()
      repPort.argKind = akReplyPort
      repPort.argName = "ReplyPort"
      repPort.argVarName = "ReplyPort"
      repPort.argMsgField = "ReplyPort"
      repPort.argType = TypeSystem.itRealReplyPortType
      repPort.argRoutine = rt
      rt.rtReplyPort = repPort
    }

    rt.rtNumRequestVar = reqPos
    rt.rtNumReplyVar = repPos
    rt.rtMaxRequestPos = reqPos
    rt.rtMaxReplyPos = repPos
    rt.rtRequestKPDs = reqKPDs
    rt.rtReplyKPDs = repKPDs

    if !hasReplyArgs {
      rt.rtNoReplyArgs = true
    }

    // Set user/server names
    rt.rtUserName = (Global.userPrefix.isEmpty ? "" : Global.userPrefix) + rt.rtName
    rt.rtServerName = (Global.serverPrefix.isEmpty ? "" : Global.serverPrefix) + rt.rtName
  }
}
