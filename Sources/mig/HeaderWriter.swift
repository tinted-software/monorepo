// HeaderWriter.swift — Generate the user-side header file

import Foundation

func writeUserHeader(fileName: String, statements: [Statement]) {
  guard let w = openIndentWriter(fileName) else { return }

  let protect = "_\(Global.subsystemName)__defs_"

  w.writeln("#ifndef\t\(protect)")
  w.writeln("#define\t\(protect)")
  w.blankLine()

  w.writeln("/* Module \(Global.subsystemName) */")
  w.blankLine()

  // Includes
  if Global.emitCountAnnotations {
    w.writeln("#include <sys/cdefs.h>")
  }
  w.writeln("#include <mach/mach_types.h>")
  w.writeln("#include <mach/message.h>")
  w.writeln("#include <mach/mig_errors.h>")
  w.writeln("#include <mach/port.h>")
  w.writeln("#include <mach/kern_return.h>")
  w.blankLine()

  // Subsystem info
  w.writeln("#define \(Global.subsystemName)__subsystem_name \"\(Global.subsystemName)\"")
  w.writeln("#define \(Global.subsystemName)__subsystem_base \(Global.subsystemBase)")
  w.blankLine()

  if !Global.serverDemux.isEmpty {
    w.writeln("extern const struct \(Global.serverSubsys) {")
    w.writeln("\tmach_msg_id_t start;")
    w.writeln("\tmach_msg_id_t end;")
    w.writeln("\tkern_return_t (*server)(mach_msg_header_t *, mach_msg_header_t *);")
    w.writeln("} \(Global.serverSubsys);")
    w.blankLine()
  }

  // Generate request/reply types
  writeRequestTypes(w, statements: statements)
  writeReplyTypes(w, statements: statements)

  // Routine declarations
  for st in statements {
    if st.stKind == .routine, let rt = st.stRoutine {
      writeUserRoutineDeclaration(w, rt: rt)
    }
  }

  w.blankLine()
  w.writeln("#endif\t/* \(protect) */")
  w.blankLine()
  w.stream.closeFile()
}

private func writeUserRoutineDeclaration(_ w: IndentWriter, rt: Routine) {
  let retType = rt.rtRetCode?.argType?.itUserType ?? "kern_return_t"
  let userName = Global.userPrefix.isEmpty ? rt.rtName : Global.userPrefix + rt.rtName

  w.blankLine()
  w.write("extern \(retType) \(userName)\n(")

  // Write arguments that are user-visible
  var firstArg = true
  var arg = rt.rtArgs
  while let a = arg {
    if akCheck(a.argKind, akbUserArg) {
      if !firstArg { w.write(",\n") }
      let star = (a.argType?.itNativePointer ?? false) ? "*" : ""
      w.write("\t\(a.argType?.itUserType ?? "void") \(star)\(a.argVarName)")
      firstArg = false
    }
    arg = a.argNext
  }

  if firstArg { w.write("\tvoid") }

  w.writeln(");")
}

// MARK: - Server header

func writeServerHeader(fileName: String, statements: [Statement]) {
  guard let w = openIndentWriter(fileName) else { return }

  let protect = "_\(Global.subsystemName)__server_"

  w.writeln("#ifndef\t\(protect)")
  w.writeln("#define\t\(protect)")
  w.blankLine()

  w.writeln("/* Module server \(Global.subsystemName) */")
  w.blankLine()
  w.writeln("#include <mach/mach_types.h>")
  w.writeln("#include <mach/message.h>")
  w.blankLine()

  // Import user header
  if !Global.userHeaderFileName.isEmpty {
    w.writeln("#include \"\(Global.userHeaderFileName)\"")
  }
  w.blankLine()

  // Server function declarations with ServerPrefix
  for st in statements {
    if st.stKind == .routine, let rt = st.stRoutine {
      let serverName = Global.serverPrefix.isEmpty ? rt.rtName : Global.serverPrefix + rt.rtName

      w.write("extern kern_return_t \(serverName)\n(")

      var firstArg = true
      var arg = rt.rtArgs
      while let a = arg {
        if akCheck(a.argKind, akbServerArg) {
          if !firstArg { w.write(",\n") }
          let star = (a.argType?.itNativePointer ?? false) ? "*" : ""
          w.write("\t\(a.argType?.itTransType ?? "void") \(star)\(a.argVarName)")
          firstArg = false
        }
        arg = a.argNext
      }
      if firstArg { w.write("\tvoid") }
      w.writeln(");\n")
    }
  }

  w.blankLine()
  w.writeln(
    "extern boolean_t \(Global.serverDemux)(mach_msg_header_t *InHeadP, mach_msg_header_t *OutHeadP);"
  )
  w.blankLine()
  w.writeln("#endif\t/* \(protect) */")
  w.stream.closeFile()
}

// MARK: - Internal header (for kernel servers)

func writeInternalHeader(fileName: String, statements: [Statement]) {
  guard let w = openIndentWriter(fileName) else { return }

  let protect = "_\(Global.subsystemName)__internal_"

  w.writeln("#ifndef\t\(protect)")
  w.writeln("#define\t\(protect)")
  w.blankLine()
  w.writeln("/* Module Internal \(Global.subsystemName) */")
  w.blankLine()

  if !Global.userHeaderFileName.isEmpty {
    w.writeln("#include \"\(Global.userHeaderFileName)\"")
  }
  w.blankLine()

  // Implementation tags
  w.writeln(
    "#define \(Global.subsystemName)_routine_count \(statements.filter { $0.stKind == .routine }.count)"
  )
  w.blankLine()

  for st in statements {
    if st.stKind == .routine, let rt = st.stRoutine {
      let implName = Global.serverPrefix.isEmpty ? rt.rtName : Global.serverPrefix + rt.rtName
      w.writeln("#define \(implName)_max \(rt.rtMaxRequestPos)")

      // Write argument tags
      var arg = rt.rtArgs
      while let a = arg {
        if akCheck(a.argKind, akbServerArg) {
          w.writeln("#define \(implName)_\(a.argMsgField) \(a.argMsgField)")
        }
        arg = a.argNext
      }
      w.blankLine()
    }
  }

  w.writeln("#endif\t/* \(protect) */")
  w.stream.closeFile()
}

// MARK: - Defines header (for msgh_ids)

func writeDefinesHeader(fileName: String, statements: [Statement]) {
  guard let w = openIndentWriter(fileName) else { return }

  let protect = "_\(Global.subsystemName)__defines_"
  w.writeln("#ifndef\t\(protect)")
  w.writeln("#define\t\(protect)")
  w.blankLine()

  for st in statements {
    if st.stKind == .routine, let rt = st.stRoutine {
      let id = Global.subsystemBase + rt.rtNumber
      w.writeln("#define MSG_\(Global.subsystemName)_\(rt.rtName) \(id)")
    }
  }

  w.blankLine()
  w.writeln("#endif\t/* \(protect) */")
  w.stream.closeFile()
}

// MARK: - Types-only header (for .defs files with no subsystem, e.g. mach_types.defs)

/// Emit a C header containing only typedef aliases derived from `type X = Y` declarations
/// in the .defs file.  This is used for files like `mach_types.defs` that declare types
/// but have no subsystem block, so normal MIG output would be empty.
func writeTypesHeader(fileName: String) {
  guard let w = openIndentWriter(fileName) else { return }

  // Derive a guard name from the file basename.
  let baseName = URL(fileURLWithPath: fileName).deletingPathExtension().lastPathComponent
  let protect = "_\(baseName)_generated_"

  w.writeln("/* Auto-generated by mig (Swift) from \(baseName).defs — DO NOT EDIT */")
  w.writeln("#ifndef \(protect)")
  w.writeln("#define \(protect)")
  w.blankLine()
  w.writeln("#include <mach/mach_types.h>")
  w.blankLine()

  // Walk every type that was declared with `type X = Y` and has a distinct user-type alias.
  // TypeSystem keeps them in a linked list; we collect all, then emit stable sorted output.
  var namedTypes: [(name: String, cType: String)] = []
  TypeSystem.enumerateDeclaredTypes { name, it in
    // Only emit types that were explicitly declared and have a non-trivial user alias.
    // Skip predefined bootstrap types (kern_return_t, mach_port_t, etc.) that come from
    // TypeSystem.initType() rather than the parsed .defs file.
    let alias = it.itUserType.isEmpty ? it.itName : it.itUserType
    if !name.isEmpty && !alias.isEmpty && name != alias {
      namedTypes.append((name: name, cType: alias))
    }
  }

  // Sort for deterministic output.
  namedTypes.sort { $0.name < $1.name }

  for (name, cType) in namedTypes {
    w.writeln("typedef \(cType) \(name);")
  }

  w.blankLine()
  w.writeln("#endif /* \(protect) */")
  w.stream.closeFile()
}
