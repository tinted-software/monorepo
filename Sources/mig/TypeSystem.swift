// TypeSystem.swift — Type checking and symbol table for MIG

import Foundation

class TypeSystem {
  // Self-organizing linked list of types
  private nonisolated(unsafe) static var typeList: IPCType? = nil
  // Set of type names that were explicitly declared in the .defs file (via `type X = Y`)
  private nonisolated(unsafe) static var explicitlyDeclared: Set<String> = []

  // Predefined types
  nonisolated(unsafe) static var itRetCodeType: IPCType? = nil
  nonisolated(unsafe) static var itNdrCodeType: IPCType? = nil
  nonisolated(unsafe) static var itDummyType: IPCType? = nil
  nonisolated(unsafe) static var itTidType: IPCType? = nil
  nonisolated(unsafe) static var itRequestPortType: IPCType? = nil
  nonisolated(unsafe) static var itZeroReplyPortType: IPCType? = nil
  nonisolated(unsafe) static var itRealReplyPortType: IPCType? = nil
  nonisolated(unsafe) static var itWaitTimeType: IPCType? = nil
  nonisolated(unsafe) static var itMsgOptionType: IPCType? = nil

  // MARK: - Symbol table

  static func lookUp(_ name: String) -> IPCType? {
    var it = typeList
    var prev: IPCType? = nil

    while it != nil {
      if it!.itName == name {
        // Move to front (self-organizing list)
        if let p = prev {
          p.itNext = it!.itNext
          it!.itNext = typeList
          typeList = it
        }
        return it
      }
      prev = it
      it = it!.itNext
    }
    return nil
  }

  static func insert(name: String, type: IPCType) {
    type.itName = name
    type.itNext = typeList
    typeList = type
  }

  // MARK: - Type creation

  static func allocType() -> IPCType {
    let it = IPCType()
    it.itStruct = true
    it.itInLine = true
    it.itNumber = 1
    return it
  }

  static func shortDecl(inname: UInt, instr: String, outname: UInt, outstr: String, dfault: UInt)
    -> IPCType
  {
    var it = allocType()
    it.itInName = inname
    it.itInNameStr = instr
    it.itOutName = outname
    it.itOutNameStr = outstr
    it.itSize = dfault

    if inname == MACH_MSG_TYPE_STRING_C && dfault == 8 {
      it.itStruct = false
      it.itString = true
    }

    let portSize: UInt = UInt(MemoryLayout<Int32>.size) * 8
    let isPort =
      (inname == MACH_MSG_TYPE_PORT_SEND || inname == MACH_MSG_TYPE_PORT_SEND_ONCE
        || inname == MACH_MSG_TYPE_PORT_RECEIVE || inname == MACH_MSG_TYPE_PORT_NAME
        || inname == MACH_MSG_TYPE_POLYMORPHIC || inname == MACH_MSG_TYPE_MOVE_SEND
        || inname == MACH_MSG_TYPE_MOVE_SEND_ONCE || inname == MACH_MSG_TYPE_COPY_SEND
        || inname == MACH_MSG_TYPE_MAKE_SEND || inname == MACH_MSG_TYPE_MAKE_SEND_ONCE
        || inname == MACH_MSG_TYPE_MOVE_RECEIVE
        || (inname != MACH_MSG_TYPE_UNSTRUCTURED && inname != MACH_MSG_TYPE_BIT
          && inname != MACH_MSG_TYPE_BOOLEAN && inname != MACH_MSG_TYPE_INTEGER_8
          && inname != MACH_MSG_TYPE_INTEGER_16 && inname != MACH_MSG_TYPE_INTEGER_32
          && inname != MACH_MSG_TYPE_INTEGER_64 && inname != MACH_MSG_TYPE_CHAR
          && inname != MACH_MSG_TYPE_BYTE && inname != MACH_MSG_TYPE_REAL_32
          && inname != MACH_MSG_TYPE_REAL_64 && inname != MACH_MSG_TYPE_STRING
          && inname != MACH_MSG_TYPE_STRING_C))

    // Actually, port types are those with specific port names
    let actualIsPort =
      (inname >= MACH_MSG_TYPE_MOVE_RECEIVE && inname <= MACH_MSG_TYPE_MAKE_SEND_ONCE)
      || inname == MACH_MSG_TYPE_PORT_NAME || inname == MACH_MSG_TYPE_PORT_RECEIVE
      || inname == MACH_MSG_TYPE_PORT_SEND || inname == MACH_MSG_TYPE_PORT_SEND_ONCE
      || inname == MACH_MSG_TYPE_POLYMORPHIC

    if actualIsPort {
      it.itPortType = true
      it.itKPD_Number = 1
    }

    calculateSizeInfo(it)
    return it
  }

  // Copy type (without name, for creating derived types)
  static func copyType(_ old: IPCType) -> IPCType {
    let new = allocType()
    new.itTypeSize = old.itTypeSize
    new.itPadSize = old.itPadSize
    new.itMinTypeSize = old.itMinTypeSize
    new.itInName = old.itInName
    new.itOutName = old.itOutName
    new.itSize = old.itSize
    new.itNumber = old.itNumber
    new.itKPD_Number = old.itKPD_Number
    new.itInLine = old.itInLine
    new.itMigInLine = old.itMigInLine
    new.itPortType = old.itPortType
    new.itInNameStr = old.itInNameStr
    new.itOutNameStr = old.itOutNameStr
    new.itStruct = old.itStruct
    new.itString = old.itString
    new.itVarArray = old.itVarArray
    new.itNoOptArray = old.itNoOptArray
    new.itNative = old.itNative
    new.itNativePointer = old.itNativePointer
    new.itElement = old.itElement != nil ? old.itElement : old  // point at old
    new.itUserType = old.itUserType
    new.itServerType = old.itServerType
    new.itTransType = old.itTransType
    new.itKPDType = old.itKPDType
    new.itInTrans = old.itInTrans
    new.itOutTrans = old.itOutTrans
    new.itDestructor = old.itDestructor
    new.itBadValue = old.itBadValue
    new.itOOL_Number = old.itOOL_Number
    return new
  }

  // Reset translation/destruction/type info
  static func resetType(_ old: IPCType) -> IPCType {
    old.itInTrans = nil
    old.itOutTrans = nil
    old.itDestructor = nil
    old.itUserType = ""
    old.itServerType = ""
    old.itTransType = ""
    return old
  }

  // type new = old;
  static func prevDecl(_ name: String) -> IPCType {
    if let old = lookUp(name) {
      return copyType(old)
    } else {
      error("type '\(name)' not defined")
      return allocType()
    }
  }

  // type new = array[*:number] of old;
  static func varArrayDecl(number: UInt, old: IPCType) -> IPCType {
    var it = resetType(copyType(old))

    if !it.itInLine {
      if it.itKPD_Number != 1 || number != 0 {
        error("IPC type decl is too complicated for Kernel Processed Data")
      }
      it.itKPD_Number *= number
      it.itNumber = 1
      it.itInLine = false
      it.itStruct = false
      it.itOOL_Number = number
    } else if it.itVarArray {
      error("IPC type decl is too complicated")
    } else if number != 0 {
      it.itNumber *= number
      it.itInLine = true
      it.itStruct = false
      if it.itPortType {
        it.itKPD_Number *= number
      }
      it.itOOL_Number = number
    } else {
      it.itNumber = 0
      it.itMigInLine = true
      it.itInLine = false
      it.itStruct = true
      it.itKPD_Number = 1
      it.itOOL_Number = 0
    }

    it.itVarArray = true
    it.itString = false
    calculateSizeInfo(it)
    return it
  }

  // type new = array[number] of old;
  static func arrayDecl(number: UInt, old: IPCType) -> IPCType {
    var it = resetType(copyType(old))

    if !it.itInLine {
      if it.itKPD_Number != 1 {
        error("IPC type decl is too complicated for Kernel Processed Data")
      }
      it.itKPD_Number *= number
      it.itNumber = 1
      it.itStruct = false
      it.itString = false
      it.itVarArray = false
    } else if it.itVarArray {
      error("IPC type decl is too complicated")
    } else {
      it.itNumber *= number
      it.itStruct = false
      it.itString = false
      if it.itPortType {
        it.itKPD_Number *= number
      }
    }

    calculateSizeInfo(it)
    return it
  }

  // type new = ^ old;
  static func ptrDecl(_ it: IPCType) -> IPCType {
    if !it.itInLine && !it.itMigInLine {
      error("IPC type decl is already defined to be Out-Of-Line")
    }
    it.itInLine = false
    it.itStruct = true
    it.itString = false
    it.itMigInLine = false
    it.itKPD_Number = 1
    calculateSizeInfo(it)
    return it
  }

  // type new = struct[number] of old;
  static func structDecl(number: UInt, old: IPCType) -> IPCType {
    var it = resetType(copyType(old))

    if !it.itInLine || it.itVarArray {
      error("IPC type decl is too complicated")
    }
    it.itNumber *= number
    it.itStruct = true
    it.itString = false
    calculateSizeInfo(it)
    return it
  }

  // c_string[count]
  static func cStringDecl(count: UInt, varying: Bool) -> IPCType {
    let element = shortDecl(
      inname: MACH_MSG_TYPE_STRING_C, instr: "MACH_MSG_TYPE_STRING_C",
      outname: MACH_MSG_TYPE_STRING_C, outstr: "MACH_MSG_TYPE_STRING_C", dfault: 8)
    typeDecl(name: "char", type: element)

    var it = resetType(copyType(element))
    it.itNumber = count
    it.itVarArray = varying
    it.itStruct = false
    it.itString = true
    calculateSizeInfo(it)
    return it
  }

  static func nativeType(_ cType: String, pointer: Bool, badVal: String?) -> IPCType {
    var it = allocType()
    it.itInName = MACH_MSG_TYPE_BYTE
    it.itInNameStr = "MACH_MSG_TYPE_BYTE"
    it.itOutName = MACH_MSG_TYPE_BYTE
    it.itOutNameStr = "MACH_MSG_TYPE_BYTE"
    it.itInLine = true
    it.itNative = true
    it.itNativePointer = pointer
    it.itServerType = cType
    it.itUserType = cType
    it.itTransType = cType
    it.itBadValue = badVal
    calculateSizeInfo(it)
    calculateNameInfo(it)
    return it
  }

  // MARK: - Helper functions

  static func calculateSizeInfo(_ it: IPCType) {
    if !it.isKernelProcData {
      let bytes = (it.itNumber * it.itSize + 7) / 8
      let padding = machine_padding(bytes)
      it.itTypeSize = bytes
      it.itPadSize = padding
      if it.isVariableSizedUntyped {
        it.itMinTypeSize = UInt(MemoryLayout<UInt32>.size)
        if it.itString {
          it.itMinTypeSize += UInt(MemoryLayout<UInt32>.size)
        }
      } else {
        it.itMinTypeSize = bytes + padding
      }
    } else {
      let bytes: UInt
      if it.isMultipleKPD {
        bytes = it.itKPD_Number * 12
      } else {
        bytes = 12
      }
      it.itTypeSize = bytes
      it.itPadSize = 0
      it.itMinTypeSize = bytes
    }

    if it.itTypeSize == 0 && !it.itVarArray && !it.itNative {
      warning("sizeof(type) == 0")
    }
  }

  static func calculateNameInfo(_ it: IPCType) {
    if it.itInNameStr.isEmpty {
      it.itInNameStr = String(it.itInName)
    }
    if it.itOutNameStr.isEmpty {
      it.itOutNameStr = String(it.itOutName)
    }
    if it.itUserType.isEmpty {
      it.itUserType = it.itName
    }
    if it.itServerType.isEmpty {
      it.itServerType = it.itName
    }
    if it.itTransType.isEmpty {
      it.itTransType = it.itServerType
    }
  }

  static func typeDecl(name: String, type: IPCType) {
    type.itName = name
    calculateNameInfo(type)
    // Mark this type as explicitly declared by the .defs source.
    explicitlyDeclared.insert(name)

    if type.itVarArray {
      if type.itInTrans != nil || type.itOutTrans != nil {
        error("\(name): can't translate variable-sized arrays")
      }
      if type.itDestructor != nil {
        error("\(name): can't destroy variable-sized array")
      }
    }
  }

  // MARK: - Special type creators

  static func makeCountType() -> IPCType {
    var it = allocType()
    it.itName = "mach_msg_type_number_t"
    it.itInName = machine_integer_size
    it.itInNameStr = machine_integer_name
    it.itOutName = machine_integer_size
    it.itOutNameStr = machine_integer_name
    it.itSize = machine_integer_bits
    calculateSizeInfo(it)
    calculateNameInfo(it)
    return it
  }

  static func makePolyType() -> IPCType {
    var it = allocType()
    it.itName = "mach_msg_type_name_t"
    it.itInName = machine_integer_size
    it.itInNameStr = machine_integer_name
    it.itOutName = machine_integer_size
    it.itOutNameStr = machine_integer_name
    it.itSize = machine_integer_bits
    calculateSizeInfo(it)
    calculateNameInfo(it)
    return it
  }

  static func makeDeallocType() -> IPCType {
    var it = allocType()
    it.itName = "boolean_t"
    it.itInName = MACH_MSG_TYPE_BOOLEAN
    it.itInNameStr = "MACH_MSG_TYPE_BOOLEAN"
    it.itOutName = MACH_MSG_TYPE_BOOLEAN
    it.itOutNameStr = "MACH_MSG_TYPE_BOOLEAN"
    it.itSize = machine_integer_bits
    calculateSizeInfo(it)
    calculateNameInfo(it)
    return it
  }

  // MARK: - Type checking helpers

  static func checkReturnType(name: String, it: IPCType) {
    if !it.itStruct {
      error("type of \(name) is too complicated")
    }
    if it.itInName == MACH_MSG_TYPE_POLYMORPHIC || it.itOutName == MACH_MSG_TYPE_POLYMORPHIC {
      error("type of \(name) can't be polymorphic")
    }
  }

  static func checkRequestPortType(name: String, it: IPCType) {
    if (it.itOutName != MACH_MSG_TYPE_PORT_SEND && it.itOutName != MACH_MSG_TYPE_PORT_SEND_ONCE
      && it.itOutName != MACH_MSG_TYPE_POLYMORPHIC) || it.itNumber != 1
      || it.itSize != PortSize
      || !it.itInLine || !it.itStruct || it.itVarArray
    {
      error("argument \(name) isn't a proper request port")
    }
  }

  static func checkReplyPortType(name: String, it: IPCType) {
    if (it.itOutName != MACH_MSG_TYPE_PORT_SEND && it.itOutName != MACH_MSG_TYPE_PORT_SEND_ONCE
      && it.itOutName != MACH_MSG_TYPE_POLYMORPHIC && it.itOutName != 0) || it.itNumber != 1
      || it.itSize != PortSize || !it.itInLine || !it.itStruct || it.itVarArray
    {
      error("argument \(name) isn't a proper reply port")
    }
  }

  static func checkIntType(name: String, it: IPCType) {
    if it.itInName != machine_integer_size || it.itOutName != machine_integer_size
      || it.itNumber != 1 || it.itSize != machine_integer_bits || !it.itInLine || !it.itStruct
      || it.itVarArray
    {
      error("argument \(name) isn't a proper integer")
    }
  }

  static func checkTokenType(name: String, it: IPCType) {
    if it.itMigInLine || it.itNoOptArray || it.itString || it.itTypeSize != 8 || !it.itInLine
      || !it.itStruct || it.itVarArray || it.itPortType
    {
      error("argument \(name) isn't a proper Token")
    }
  }

  // MARK: - Enumeration

  /// Iterate over every type that was explicitly declared via `type X = Y` in the .defs file.
  static func enumerateDeclaredTypes(_ body: (String, IPCType) -> Void) {
    var it = typeList
    while let current = it {
      if explicitlyDeclared.contains(current.itName) {
        body(current.itName, current)
      }
      it = current.itNext
    }
  }

  // MARK: - Initialization

  static func initType() {
    let size = UInt(MemoryLayout<Int>.size * 8)
    if size == 32 {
      machine_integer_name = "MACH_MSG_TYPE_INTEGER_32"
      machine_integer_size = MACH_MSG_TYPE_INTEGER_32
    } else if size == 64 {
      machine_integer_name = "MACH_MSG_TYPE_INTEGER_64"
      machine_integer_size = MACH_MSG_TYPE_INTEGER_64
    } else {
      error("init_type unknown size \(size)")
    }
    machine_integer_bits = size

    // itRetCodeType
    itRetCodeType = allocType()
    itRetCodeType!.itName = "kern_return_t"
    itRetCodeType!.itInName = machine_integer_size
    itRetCodeType!.itInNameStr = machine_integer_name
    itRetCodeType!.itOutName = machine_integer_size
    itRetCodeType!.itOutNameStr = machine_integer_name
    itRetCodeType!.itSize = machine_integer_bits
    calculateSizeInfo(itRetCodeType!)
    calculateNameInfo(itRetCodeType!)

    // itRequestPortType
    itRequestPortType = allocType()
    itRequestPortType!.itName = "mach_port_t"
    itRequestPortType!.itInName = MACH_MSG_TYPE_COPY_SEND
    itRequestPortType!.itInNameStr = "MACH_MSG_TYPE_COPY_SEND"
    itRequestPortType!.itOutName = MACH_MSG_TYPE_PORT_SEND
    itRequestPortType!.itOutNameStr = "MACH_MSG_TYPE_PORT_SEND"
    itRequestPortType!.itSize = PortSize
    calculateSizeInfo(itRequestPortType!)
    calculateNameInfo(itRequestPortType!)

    // itZeroReplyPortType
    itZeroReplyPortType = allocType()
    itZeroReplyPortType!.itName = "mach_port_t"
    itZeroReplyPortType!.itInName = 0
    itZeroReplyPortType!.itInNameStr = "0"
    itZeroReplyPortType!.itOutName = 0
    itZeroReplyPortType!.itOutNameStr = "0"
    itZeroReplyPortType!.itSize = PortSize
    calculateSizeInfo(itZeroReplyPortType!)
    calculateNameInfo(itZeroReplyPortType!)

    // itRealReplyPortType
    itRealReplyPortType = allocType()
    itRealReplyPortType!.itName = "mach_port_t"
    itRealReplyPortType!.itInName = MACH_MSG_TYPE_MAKE_SEND_ONCE
    itRealReplyPortType!.itInNameStr = "MACH_MSG_TYPE_MAKE_SEND_ONCE"
    itRealReplyPortType!.itOutName = MACH_MSG_TYPE_PORT_SEND_ONCE
    itRealReplyPortType!.itOutNameStr = "MACH_MSG_TYPE_PORT_SEND_ONCE"
    itRealReplyPortType!.itSize = PortSize
    calculateSizeInfo(itRealReplyPortType!)
    calculateNameInfo(itRealReplyPortType!)

    itWaitTimeType = makeCountType()
    itMsgOptionType = makeCountType()
  }
}

// Extend IPCType with mutable modify helpers
extension IPCType {
  func mutableCopy() -> IPCType {
    let new = IPCType()
    new.itNext = self.itNext
    new.itTypeSize = self.itTypeSize
    new.itPadSize = self.itPadSize
    new.itMinTypeSize = self.itMinTypeSize
    new.itInName = self.itInName
    new.itOutName = self.itOutName
    new.itSize = self.itSize
    new.itNumber = self.itNumber
    new.itKPD_Number = self.itKPD_Number
    new.itInLine = self.itInLine
    new.itMigInLine = self.itMigInLine
    new.itPortType = self.itPortType
    new.itInNameStr = self.itInNameStr
    new.itOutNameStr = self.itOutNameStr
    new.itStruct = self.itStruct
    new.itString = self.itString
    new.itVarArray = self.itVarArray
    new.itNoOptArray = self.itNoOptArray
    new.itNative = self.itNative
    new.itNativePointer = self.itNativePointer
    new.itElement = self.itElement
    new.itUserType = self.itUserType
    new.itServerType = self.itServerType
    new.itTransType = self.itTransType
    new.itKPDType = self.itKPDType
    new.itInTrans = self.itInTrans
    new.itOutTrans = self.itOutTrans
    new.itDestructor = self.itDestructor
    new.itBadValue = self.itBadValue
    new.itOOL_Number = self.itOOL_Number
    return new
  }
}
