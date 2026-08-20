// SPDX-License-Identifier: MIT
// Server-side stub generation (mirrors server.c)

use std::io::{self, Write};

use super::utils::{write_identification, write_imports, write_mig_external};
use crate::ast::{Direction, ImportKind, RoutineKind, Statement};
use crate::global::Options;

pub fn write_server(w: &mut dyn Write, stmts: &[Statement], opts: &Options) -> io::Result<()> {
    let subsys = opts.subsystem_name.as_deref().unwrap_or("?");
    let demux = opts.server_demux.as_deref().unwrap_or(subsys);

    write_identification(w, opts)?;
    writeln!(w)?;
    writeln!(w, "/* Module {subsys} */")?;
    writeln!(w)?;
    writeln!(w, "#define\t__MIG_check__server_{subsys}_subsystem__")?;
    writeln!(w)?;
    writeln!(w, "#include <mach/mach_types.h>")?;
    writeln!(w, "#include <mach/message.h>")?;
    writeln!(w, "#include <mach/ndr.h>")?;
    writeln!(w, "#include <mach/mig.h>")?;
    writeln!(w, "#include <mach/mig_errors.h>")?;
    writeln!(w)?;
    // Real Apple migcom emits this boilerplate too (migcom.tproj/utils.c's
    // WriteBogusDefines): `struct routine_descriptor`'s max_reply_msg field
    // wants a word-aligned size and mach/mig.h itself doesn't define the
    // alignment helper.
    writeln!(w, "#if !defined(_WALIGN)")?;
    writeln!(w, "#define _WALIGN(x) (((x) + 3) & ~3)")?;
    writeln!(w, "#endif /* !defined(_WALIGN) */")?;
    writeln!(w)?;

    if let Some(sheader) = opts.server_header_filename.as_deref() {
        if sheader != "/dev/null" {
            let base = sheader.rsplit('/').next().unwrap_or(sheader);
            writeln!(w, "#include \"{base}\"")?;
            writeln!(w)?;
        }
    }

    writeln!(w, "/* Includes from Import / SImport directives */")?;
    write_imports(w, stmts, &[ImportKind::Import, ImportKind::SImport])?;
    writeln!(w)?;

    // Count routines
    let routines: Vec<&crate::ast::Routine> = stmts
        .iter()
        .filter_map(|s| {
            if let Statement::Routine(r) = s {
                Some(r)
            } else {
                None
            }
        })
        .collect();

    let base = opts.subsystem_base;

    // Write per-routine dispatch functions
    for (seq, rt) in routines.iter().enumerate() {
        write_server_routine(w, rt, base + seq as u32, opts)?;
    }

    // Write the demux function
    write_demux(w, &routines, base, demux, opts)?;

    // Write the MIG subsystem descriptor
    write_subsystem(w, &routines, base, subsys, opts)?;

    Ok(())
}

fn write_server_routine(
    w: &mut dyn Write,
    rt: &crate::ast::Routine,
    _msg_id: u32,
    opts: &Options,
) -> io::Result<()> {
    let is_simple = rt.kind == RoutineKind::SimpleRoutine;
    let srv_fn = format!("{}{}", opts.server_prefix, rt.name);
    let dispatch_fn = format!("_X{}", rt.name);

    writeln!(w)?;
    writeln!(w, "/* {dispatch_fn} — dispatch wrapper for {srv_fn} */")?;

    write_mig_external(w)?;
    writeln!(w, "kern_return_t {dispatch_fn}(")?;
    writeln!(w, "\tmach_msg_header_t *InHeadP,")?;
    writeln!(w, "\tmach_msg_header_t *OutHeadP)")?;
    writeln!(w, "{{")?;

    // Request struct
    writeln!(w, "\ttypedef struct {{")?;
    writeln!(w, "\t\tmach_msg_header_t Head;")?;
    writeln!(w, "\t\tNDR_record_t NDR;")?;
    for arg in rt.args.iter().filter(|a| {
        matches!(
            a.direction,
            Direction::In | Direction::InOut | Direction::None
        )
    }) {
        let ty = arg.ty.server_type.as_deref().unwrap_or("int");
        writeln!(w, "\t\t{ty} {};\t/* in */", arg.name)?;
    }
    writeln!(w, "\t}} Request;")?;
    writeln!(w)?;

    if !is_simple {
        // Reply struct
        writeln!(w, "\ttypedef struct {{")?;
        writeln!(w, "\t\tmach_msg_header_t Head;")?;
        writeln!(w, "\t\tNDR_record_t NDR;")?;
        writeln!(w, "\t\tmach_msg_type_name_t RetCodeType;")?;
        writeln!(w, "\t\tkern_return_t RetCode;")?;
        for arg in rt
            .args
            .iter()
            .filter(|a| matches!(a.direction, Direction::Out | Direction::InOut))
        {
            let ty = arg.ty.server_type.as_deref().unwrap_or("int");
            writeln!(w, "\t\t{ty} {};\t/* out */", arg.name)?;
        }
        writeln!(w, "\t}} Reply;")?;
        writeln!(w)?;
    }

    writeln!(w, "\tRequest *In0P = (Request *)InHeadP;")?;
    if !is_simple {
        writeln!(w, "\tReply *OutP = (Reply *)OutHeadP;")?;
    }
    writeln!(w)?;

    // Real Apple migcom's `intran:`/`outtran:` clauses convert the wire
    // mach_port_t to/from the kernel object type the routine implementation
    // actually takes (e.g. `task_t convert_port_to_task_mig(mach_port_t)`);
    // the Request/Reply structs above always hold the wire (server_type)
    // representation, so any arg with a translation needs a locally-typed
    // variable to bridge the two.
    let mut call_args: Vec<String> = Vec::new();
    for arg in rt.args.iter() {
        let has_in = matches!(arg.direction, Direction::In | Direction::InOut | Direction::None);
        let has_out = matches!(arg.direction, Direction::Out | Direction::InOut);
        let translated = arg.ty.in_trans.is_some() || arg.ty.out_trans.is_some();
        let name = &arg.name;

        if translated {
            let user_ty = arg
                .ty
                .trans_type
                .as_deref()
                .or(arg.ty.user_type.as_deref())
                .unwrap_or("int");
            if has_in {
                if let Some(intran) = arg.ty.in_trans.as_deref() {
                    writeln!(w, "\t{user_ty} {name} = {intran}(In0P->{name});")?;
                } else {
                    writeln!(w, "\t{user_ty} {name};")?;
                }
            } else {
                writeln!(w, "\t{user_ty} {name};")?;
            }
            call_args.push(if has_out { format!("&{name}") } else { name.clone() });
        } else if has_out {
            if has_in {
                // Untranslated InOut: seed the reply field with the request
                // value so the callee sees the caller-supplied input.
                writeln!(w, "\tOutP->{name} = In0P->{name};")?;
            }
            call_args.push(format!("&OutP->{name}"));
        } else {
            call_args.push(format!("In0P->{name}"));
        }
    }

    if is_simple {
        writeln!(w, "\t(void){srv_fn}({});", call_args.join(", "))?;
    } else {
        writeln!(w, "\tOutP->RetCode = {srv_fn}({});", call_args.join(", "))?;
    }

    // Convert translated Out/InOut results back to the wire representation.
    for arg in rt.args.iter() {
        let has_out = matches!(arg.direction, Direction::Out | Direction::InOut);
        if !has_out {
            continue;
        }
        if let Some(outtran) = arg.ty.out_trans.as_deref() {
            writeln!(w, "\tOutP->{} = {outtran}({});", arg.name, arg.name)?;
        }
    }

    if !is_simple {
        writeln!(w)?;
        writeln!(
            w,
            "\tOutP->Head.msgh_size = (mach_msg_size_t)sizeof(Reply);"
        )?;
    }

    writeln!(w, "\treturn MACH_MSG_SUCCESS;")?;
    writeln!(w, "}}")
}

fn write_demux(
    w: &mut dyn Write,
    routines: &[&crate::ast::Routine],
    base: u32,
    demux: &str,
    _opts: &Options,
) -> io::Result<()> {
    writeln!(w)?;
    writeln!(w, "/* Server demux */")?;
    write_mig_external(w)?;
    writeln!(w, "boolean_t {demux}(")?;
    writeln!(w, "\tmach_msg_header_t *InHeadP,")?;
    writeln!(w, "\tmach_msg_header_t *OutHeadP)")?;
    writeln!(w, "{{")?;
    writeln!(w, "\tmach_msg_id_t msgh_id = InHeadP->msgh_id;")?;
    writeln!(
        w,
        "\tif (msgh_id < {base} || msgh_id > {})",
        base + routines.len() as u32 - 1
    )?;
    writeln!(w, "\t\treturn FALSE;")?;
    writeln!(w)?;
    writeln!(w, "\ttypedef kern_return_t (*dispatch_fn_t)(")?;
    writeln!(w, "\t\tmach_msg_header_t *, mach_msg_header_t *);")?;
    writeln!(w)?;
    writeln!(w, "\tstatic const dispatch_fn_t dispatch_table[] = {{")?;
    for rt in routines.iter() {
        writeln!(w, "\t\t(dispatch_fn_t)_X{},", rt.name)?;
    }
    writeln!(w, "\t}};")?;
    writeln!(w)?;
    writeln!(
        w,
        "\treturn dispatch_table[msgh_id - {base}](InHeadP, OutHeadP) == MACH_MSG_SUCCESS;"
    )?;
    writeln!(w, "}}")
}

fn write_subsystem(
    w: &mut dyn Write,
    routines: &[&crate::ast::Routine],
    base: u32,
    subsys: &str,
    _opts: &Options,
) -> io::Result<()> {
    let subsys_sym = format!("{}_subsystem", subsys);
    let selector = format!("_{subsys}_server_routine");

    // `struct mig_subsystem`'s first field (mach/mig.h) is a routine
    // selector callback, used by the generic IPC kobject dispatcher to look
    // up a routine's stub function from a message id without going through
    // the per-subsystem demux; real migcom generates one per subsystem.
    writeln!(w)?;
    writeln!(w, "/* Routine selector for {subsys_sym} */")?;
    write_mig_external(w)?;
    writeln!(w, "mig_routine_t {selector}(mach_msg_header_t *InHeadP)")?;
    writeln!(w, "{{")?;
    writeln!(
        w,
        "\tint msgh_id = ((mach_msg_id_t)InHeadP->msgh_id) - {base};"
    )?;
    writeln!(w, "\tif ((msgh_id >= 0) && (msgh_id < {}))", routines.len())?;
    writeln!(
        w,
        "\t\treturn (mig_routine_t){subsys_sym}.routine[msgh_id].stub_routine;"
    )?;
    writeln!(w, "\treturn (mig_routine_t) 0;")?;
    writeln!(w, "}}")?;

    writeln!(w)?;
    writeln!(w, "/* MIG subsystem descriptor */")?;
    writeln!(w, "const struct mig_subsystem {subsys_sym} = {{")?;
    writeln!(w, "\t{selector},\t/* server */")?;
    writeln!(w, "\t{},\t/* start */", base)?;
    writeln!(w, "\t{},\t/* end */", base + routines.len() as u32)?;
    writeln!(w, "\tsizeof(mig_reply_error_t),\t/* maxsize */")?;
    writeln!(w, "\t(vm_address_t)0,\t/* reserved */")?;
    writeln!(w, "\t{{")?;
    for rt in routines.iter() {
        writeln!(
            w,
            "\t\t{{ (mig_impl_routine_t)0, (mig_stub_routine_t)_X{name}, 0, 0, (routine_arg_descriptor_t)0, _WALIGN(sizeof(mig_reply_error_t)) }},",
            name = rt.name
        )?;
    }
    writeln!(w, "\t}}")?;
    writeln!(w, "}};")
}
