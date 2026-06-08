// Lexer.swift — Hand-written lexer for MIG .defs files

import Foundation

// MARK: - Token type

enum Token: Equatable {
  case skip
  case routine
  case simpleRoutine
  case subsystem
  case kernelUser
  case kernelServer
  case msgOption
  case useSpecialReplyPort
  case consumeOnSendError
  case msgSeqno
  case waitTime
  case sendTime
  case noWaitTime
  case noSendTime
  case errorProc
  case serverPrefix
  case userPrefix
  case serverDemux
  case rcsId

  case import_
  case uImport
  case sImport
  case iImport
  case dImport

  case in_
  case out
  case inOut
  case userImpl
  case serverImpl
  case requestPort
  case replyPort
  case sReplyPort
  case uReplyPort

  case type_
  case array
  case struct_
  case of

  case inTran
  case outTran
  case destructor
  case cType
  case cUserType
  case userTypeLimit
  case onStackLimit
  case cServerType
  case pointerTo
  case pointerToIfNot
  case valueOf

  case cString
  case secToken
  case userSecToken
  case serverSecToken
  case auditToken
  case userAuditToken
  case serverAuditToken
  case serverContextToken

  case colon  // :
  case semi  // ;
  case comma  // ,
  case plus  // +
  case minus  // -
  case star  // *
  case div  // /
  case lParen  // (
  case rParen  // )
  case equal  // =
  case caret  // ^
  case tilde  // ~
  case lAngle  // <
  case rAngle  // >
  case lBrack  // [
  case rBrack  // ]
  case bar  // |

  case number(UInt)
  case symbolicType(innumber: UInt, instr: String, outnumber: UInt, outstr: String, size: UInt)
  case identifier(String)
  case string(String)
  case qString(String)
  case fileName(String)
  case ipcFlag(IPCTypeFlags)

  case eof
  case illegal(Character)
}

// MARK: - Lexer states

enum LexerState {
  case normal
  case stringMode  // after LookString()
  case fileNameMode  // after LookFileName()
  case qStringMode  // after LookQString()
  case skipToEOL  // after # directive
  case done
}

// MARK: - Lexer

class Lexer {
  private let input: String
  private var index: String.Index
  private var state: LexerState = .normal
  private var savedState: LexerState = .normal
  private var lastToken: Token = .eof

  init(input: String) {
    self.input = input
    self.index = input.startIndex
  }

  // MARK: - Lexer state management

  func lookNormal() {
    savedState = state
    state = .normal
  }

  func lookString() {
    savedState = state
    state = .stringMode
  }

  func lookQString() {
    savedState = state
    state = .qStringMode
  }

  func lookFileName() {
    savedState = state
    state = .fileNameMode
  }

  private func restoreState() {
    state = savedState
  }

  // MARK: - Character helpers

  private var isAtEnd: Bool { index >= input.endIndex }

  private func peek() -> Character? {
    guard index < input.endIndex else { return nil }
    return input[index]
  }

  private func advance() -> Character? {
    guard index < input.endIndex else { return nil }
    let c = input[index]
    index = input.index(after: index)
    return c
  }

  private func match(_ expected: Character) -> Bool {
    guard index < input.endIndex, input[index] == expected else { return false }
    index = input.index(after: index)
    return true
  }

  // MARK: - Lexer main

  func nextToken() -> Token {
    if isAtEnd { return .eof }

    switch state {
    case .normal: return lexNormal()
    case .stringMode: return lexString()
    case .fileNameMode: return lexFileName()
    case .qStringMode: return lexQString()
    case .skipToEOL: return lexSkipToEOL()
    case .done: return .eof
    }
  }

  func allTokens() -> [Token] {
    var tokens: [Token] = []
    while true {
      let tok = nextToken()
      tokens.append(tok)
      if case .eof = tok { break }
    }
    return tokens
  }

  // MARK: - Normal mode

  private func lexNormal() -> Token {
    skipWhitespace()

    guard let c = peek() else { return .eof }

    switch c {
    case "\n":
      _ = advance()
      lineno += 1
      return lexNormal()

    case ":":
      _ = advance()
      return .colon
    case ";":
      _ = advance()
      return .semi
    case ",":
      _ = advance()
      return .comma
    case "+":
      _ = advance()
      return .plus
    case "-":
      _ = advance()
      return .minus
    case "*":
      _ = advance()
      return .star
    case "/":
      _ = advance()
      return .div
    case "(":
      _ = advance()
      return .lParen
    case ")":
      _ = advance()
      return .rParen
    case "=":
      _ = advance()
      return .equal
    case "^":
      _ = advance()
      return .caret
    case "~":
      _ = advance()
      return .tilde
    case "<":
      _ = advance()
      return .lAngle
    case ">":
      _ = advance()
      return .rAngle
    case "[":
      _ = advance()
      return .lBrack
    case "]":
      _ = advance()
      return .rBrack
    case "|":
      _ = advance()
      return .bar

    case "#":
      _ = advance()
      handleSharpDirective()
      return lexNormal()

    case "0"..."9":
      return lexNumber()

    case "A"..."Z", "a"..."z", "_":
      return lexIdentifierOrKeyword()

    default:
      _ = advance()
      return .illegal(c)
    }
  }

  // MARK: - Special modes

  private func lexString() -> Token {
    // In String mode, match a sequence of [-/._$A-Za-z0-9]+
    var str = ""
    while let c = peek() {
      if c.isLetter || c.isNumber || c == "-" || c == "/" || c == "." || c == "_" || c == "$" {
        str.append(advance()!)
      } else {
        break
      }
    }
    if str.isEmpty {
      restoreState()
      return lexNormal()
    }
    restoreState()
    return .string(str)
  }

  private func lexFileName() -> Token {
    // Filename is either a quoted string or an angle-bracket string
    skipWhitespace()
    guard let c = peek() else {
      restoreState()
      return .eof
    }

    if c == "\"" {
      _ = advance()
      var str = ""
      while let ch = peek(), ch != "\"" && ch != "\n" {
        str.append(advance()!)
      }
      if match("\"") {}
      restoreState()
      return .fileName("\"" + str + "\"")
    } else if c == "<" {
      _ = advance()
      var str = ""
      while let ch = peek(), ch != ">" && ch != "\n" {
        str.append(advance()!)
      }
      if match(">") {}
      restoreState()
      return .fileName("<" + str + ">")
    }
    restoreState()
    return lexNormal()
  }

  private func lexQString() -> Token {
    skipWhitespace()
    guard let c = peek(), c == "\"" else {
      restoreState()
      return .eof
    }

    _ = advance()
    var str = ""
    while let ch = peek(), ch != "\"" && ch != "\n" {
      str.append(advance()!)
    }
    if match("\"") {}
    restoreState()
    return .qString("\"" + str + "\"")
  }

  private func lexSkipToEOL() -> Token {
    while let c = peek(), c != "\n" {
      _ = advance()
    }
    if match("\n") {
      lineno += 1
    }
    restoreState()
    return lexNormal()
  }

  // MARK: - Numbers

  private func lexNumber() -> Token {
    var str = ""
    while let c = peek(), c.isNumber {
      str.append(advance()!)
    }
    if let n = UInt(str) {
      return .number(n)
    }
    return .illegal(str.first ?? "?")
  }

  // MARK: - Identifiers and keywords

  private static let keywords: [String: Token] = [
    "routine": .routine,
    "simpleroutine": .simpleRoutine,
    "subsystem": .subsystem,
    "kerneluser": .kernelUser,
    "kernelserver": .kernelServer,
    "msgoption": .msgOption,
    "usespecialreplyport": .useSpecialReplyPort,
    "consumeonsenderror": .consumeOnSendError,
    "msgseqno": .msgSeqno,
    "waittime": .waitTime,
    "sendtime": .sendTime,
    "nowaittime": .noWaitTime,
    "nosendtime": .noSendTime,
    "error": .errorProc,
    "serverprefix": .serverPrefix,
    "userprefix": .userPrefix,
    "serverdemux": .serverDemux,
    "rcsid": .rcsId,
    "import": .import_,
    "uimport": .uImport,
    "simport": .sImport,
    "iimport": .iImport,
    "dimport": .dImport,
    "in": .in_,
    "out": .out,
    "inout": .inOut,
    "userimpl": .userImpl,
    "serverimpl": .serverImpl,
    "requestport": .requestPort,
    "replyport": .replyPort,
    "sreplyport": .sReplyPort,
    "ureplyport": .uReplyPort,
    "type": .type_,
    "array": .array,
    "struct": .struct_,
    "of": .of,
    "intran": .inTran,
    "outtran": .outTran,
    "destructor": .destructor,
    "ctype": .cType,
    "cusertype": .cUserType,
    "usertypelimit": .userTypeLimit,
    "onstacklimit": .onStackLimit,
    "cservertype": .cServerType,
    "pointerto": .pointerTo,
    "pointertoifnot": .pointerToIfNot,
    "valueof": .valueOf,
    "c_string": .cString,
    "sectoken": .secToken,
    "serversectoken": .serverSecToken,
    "usersectoken": .userSecToken,
    "audittoken": .auditToken,
    "serveraudittoken": .serverAuditToken,
    "useraudittoken": .userAuditToken,
    "servercontexttoken": .serverContextToken,
    "skip": .skip,
  ]

  private static let ipcFlags: [(String, IPCTypeFlags)] = [
    ("samecount", .sameCount),
    ("retcode", .retCode),
    ("physicalcopy", .physicalCopy),
    ("overwrite", .overwrite),
    ("dealloc", .dealloc),
    ("notdealloc", .notDealloc),
    ("countinout", .countInOut),
    ("polymorphic", []),
    ("auto", .autoFlag),
    ("const", .constFlag),
  ]

  private static let symbolicTypes: [(String, UInt, UInt, UInt, UInt)] = [
    // name, inType, outType, specialOutType, size
    ("mach_msg_type_unstructured", MACH_MSG_TYPE_UNSTRUCTURED, MACH_MSG_TYPE_UNSTRUCTURED, 0, 0),
    ("mach_msg_type_bit", MACH_MSG_TYPE_BIT, MACH_MSG_TYPE_BIT, 0, 1),
    ("mach_msg_type_boolean", MACH_MSG_TYPE_BOOLEAN, MACH_MSG_TYPE_BOOLEAN, 0, 32),
    ("mach_msg_type_integer_8", MACH_MSG_TYPE_INTEGER_8, MACH_MSG_TYPE_INTEGER_8, 0, 8),
    ("mach_msg_type_integer_16", MACH_MSG_TYPE_INTEGER_16, MACH_MSG_TYPE_INTEGER_16, 0, 16),
    ("mach_msg_type_integer_32", MACH_MSG_TYPE_INTEGER_32, MACH_MSG_TYPE_INTEGER_32, 0, 32),
    ("mach_msg_type_integer_64", MACH_MSG_TYPE_INTEGER_64, MACH_MSG_TYPE_INTEGER_64, 0, 64),
    ("mach_msg_type_real_32", MACH_MSG_TYPE_REAL_32, MACH_MSG_TYPE_REAL_32, 0, 32),
    ("mach_msg_type_real_64", MACH_MSG_TYPE_REAL_64, MACH_MSG_TYPE_REAL_64, 0, 64),
    ("mach_msg_type_char", MACH_MSG_TYPE_CHAR, MACH_MSG_TYPE_CHAR, 0, 8),
    ("mach_msg_type_byte", MACH_MSG_TYPE_BYTE, MACH_MSG_TYPE_BYTE, 0, 8),
    (
      "mach_msg_type_move_receive", MACH_MSG_TYPE_MOVE_RECEIVE, MACH_MSG_TYPE_PORT_RECEIVE, 0,
      PortSize
    ),
    ("mach_msg_type_copy_send", MACH_MSG_TYPE_COPY_SEND, MACH_MSG_TYPE_PORT_SEND, 0, PortSize),
    ("mach_msg_type_make_send", MACH_MSG_TYPE_MAKE_SEND, MACH_MSG_TYPE_PORT_SEND, 0, PortSize),
    ("mach_msg_type_move_send", MACH_MSG_TYPE_MOVE_SEND, MACH_MSG_TYPE_PORT_SEND, 0, PortSize),
    (
      "mach_msg_type_make_send_once", MACH_MSG_TYPE_MAKE_SEND_ONCE, MACH_MSG_TYPE_PORT_SEND_ONCE, 0,
      PortSize
    ),
    (
      "mach_msg_type_move_send_once", MACH_MSG_TYPE_MOVE_SEND_ONCE, MACH_MSG_TYPE_PORT_SEND_ONCE, 0,
      PortSize
    ),
    ("mach_msg_type_port_name", MACH_MSG_TYPE_PORT_NAME, MACH_MSG_TYPE_PORT_NAME, 0, PortSize),
    (
      "mach_msg_type_port_receive", MACH_MSG_TYPE_POLYMORPHIC, MACH_MSG_TYPE_PORT_RECEIVE, 0,
      PortSize
    ),
    ("mach_msg_type_port_send", MACH_MSG_TYPE_POLYMORPHIC, MACH_MSG_TYPE_PORT_SEND, 0, PortSize),
    (
      "mach_msg_type_port_send_once", MACH_MSG_TYPE_POLYMORPHIC, MACH_MSG_TYPE_PORT_SEND_ONCE, 0,
      PortSize
    ),
    (
      "mach_msg_type_polymorphic", MACH_MSG_TYPE_POLYMORPHIC, MACH_MSG_TYPE_POLYMORPHIC, 0, PortSize
    ),
  ]

  private func lexIdentifierOrKeyword() -> Token {
    var str = ""
    while let c = peek(), c.isLetter || c.isNumber || c == "_" {
      str.append(advance()!)
    }

    let lower = str.lowercased()

    // Check keywords first
    if let tok = Lexer.keywords[lower] {
      return tok
    }

    // Check symbolic types
    for (name, inType, outType, _, size) in Lexer.symbolicTypes {
      if lower == name.lowercased() {
        let instr = String(inType)
        let outstr = String(outType)
        return .symbolicType(
          innumber: inType, instr: instr, outnumber: outType, outstr: outstr, size: size)
      }
    }

    // Check IPC flags
    for (name, flag) in Lexer.ipcFlags {
      if lower == name.lowercased() {
        if flag.isEmpty {
          // polymorphic has special handling
          return .symbolicType(
            innumber: MACH_MSG_TYPE_POLYMORPHIC, instr: "MACH_MSG_TYPE_POLYMORPHIC",
            outnumber: MACH_MSG_TYPE_POLYMORPHIC, outstr: "MACH_MSG_TYPE_POLYMORPHIC",
            size: PortSize)
        }
        return .ipcFlag(flag)
      }
    }

    return .identifier(str)
  }

  // MARK: - Helper methods

  private func skipWhitespace() {
    while let c = peek() {
      if c == " " || c == "\t" {
        _ = advance()
      } else if c == "/" {
        // Check for comment
        let nextIdx = input.index(after: index)
        if nextIdx < input.endIndex {
          if input[nextIdx] == "*" {
            skipBlockComment()
            continue
          } else if input[nextIdx] == "/" {
            skipLineComment()
            continue
          }
        }
        break
      } else {
        break
      }
    }
  }

  private func skipBlockComment() {
    _ = advance()  // skip /
    _ = advance()  // skip *
    while let c = advance() {
      if c == "*" && match("/") {
        return
      }
      if c == "\n" { lineno += 1 }
    }
  }

  private func skipLineComment() {
    _ = advance()  // skip /
    _ = advance()  // skip /
    while let c = advance() {
      if c == "\n" {
        lineno += 1
        return
      }
    }
  }

  private func handleSharpDirective() {
    skipWhitespace()

    // Read the directive body up to the first number
    var numStr = ""
    while let c = peek(), c.isNumber {
      numStr.append(advance()!)
    }

    if let lineNum = Int(numStr) {
      lineno = lineNum
    }

    // Look for a filename string
    skipWhitespace()
    if match("\"") {
      var fn = ""
      while let c = peek(), c != "\"" && c != "\n" {
        fn.append(advance()!)
      }
      if match("\"") {
        yyinname = fn
      }
    }

    state = .skipToEOL
  }
}
