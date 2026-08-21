//! EL2 boot entry, exception vector table, and trap-frame save/restore,
//! as one `global_asm!` block.
//!
//! Deliberately consolidated into a single string rather than split
//! across separate `.S` files with a GNU-as `.include` chain: that split
//! exists elsewhere in this workspace (kernel-lib's
//! `arch/aarch64/{vectors,context_macros}.S`) but isn't actually wired
//! into the buck2 build (`kernel-lib`'s `srcs` glob only matches `*.rs`
//! under `src/`, and nothing `include_str!`s those files) - the *real*
//! boot/vector code that ships lives inline in
//! `kernel-lib/src/arch/aarch64/mod.rs`'s own `global_asm!`. This module
//! follows that same (apparently the actually-supported) pattern rather
//! than repeating the orphaned split.

// gs201's per-CPU-cluster hardware watchdogs (see `wdt.rs`'s module doc
// comment) are armed by ABL across handoff and, left untouched, fire a
// platform reset a few seconds later regardless of what happens
// afterwards - including if this core never reaches Rust at all (e.g.
// falls into `hv_no_el2_hang` below because EL2 wasn't actually granted).
// `wdt.rs`'s Rust-level `disable_all()` call in `hvMain` only covers the
// "reached Rust" case; this covers every path, including ones that
// never do, as the literal first instructions executed on any core.
// Clobbers x0 only, which is already saved into x20 by the time this
// runs. Idempotent, so redundant per-core execution is harmless.
#[cfg(feature = "gs201")]
macro_rules! wdt_disable_asm {
    () => {
        r#"
    movz x0, #0x0000
    movk x0, #0x1006, lsl #16
    str wzr, [x0]
    movz x0, #0x0000
    movk x0, #0x1007, lsl #16
    str wzr, [x0]
"#
    };
}
#[cfg(not(feature = "gs201"))]
macro_rules! wdt_disable_asm {
    () => {
        ""
    };
}

core::arch::global_asm!(concat!(
    r#"
.section .text.boot
.global _start
_start:
    nop                     // code0
    b real_start            // code1
    .quad 0x1000000         // text_offset - HV_LOAD_ADDR (0x01000000), RAM base 0
    .quad __image_size
    .quad 0
    .quad 0
    .quad 0
    .quad 0
    .ascii "ARM\x64"
    .word 0

real_start:
    mov x20, x0
"#,
    wdt_disable_asm!(),
    r#"
    // MMU/caches off before any MMIO. ABL is supposed to enter the
    // Image with the MMU already off; if it does not, identity-mapped
    // DRAM still works and Device MMIO (DPU/WDT) becomes reachable.
    mrs x0, CurrentEL
    lsr x0, x0, #2
    cmp x0, #2
    b.ne 1f
    mrs x0, sctlr_el2
    movz x1, #0x1005
    bic x0, x0, x1
    msr sctlr_el2, x0
    isb
    b 2f
1:
    cmp x0, #1
    b.ne 2f
    mrs x0, sctlr_el1
    movz x1, #0x1005
    bic x0, x0, x1
    msr sctlr_el1, x0
    isb
2:
    ic iallu
    dsb sy
    isb

    mrs x0, mpidr_el1
    and x0, x0, #0xff
    cbz x0, primary_core

secondary_park:
    adrp x1, hv_smp_wake_flag
    add x1, x1, :lo12:hv_smp_wake_flag
    ldr x2, [x1]
    cbnz x2, secondary_core
    wfe
    b secondary_park

secondary_core:
    mov x9, x0
    msr cptr_el2, xzr
    isb

    adrp x10, __secondary_stacks_bottom
    add x10, x10, :lo12:__secondary_stacks_bottom
    mov x1, #0x10000
    madd x10, x9, x1, x10
    add x10, x10, x1
    mov sp, x10

    mov x0, x9
    bl hvSecondaryMain

secondary_hang:
    wfe
    b secondary_hang

primary_core:
    mrs x0, CurrentEL
    lsr x0, x0, #2
    cmp x0, #2
    b.eq from_el2
    // Pixel ABL can hand off a `fastboot boot` Image at EL1 (pKVM
    // holds EL2). Display adoption is MMIO and works at EL1; hanging
    // here leaves the Google logo forever with no other diagnostic.
    cmp x0, #1
    b.eq from_el2
    b hv_no_el2_hang

from_el2:
    mrs x0, CurrentEL
    lsr x0, x0, #2
    cmp x0, #2
    b.ne skip_cptr_el2
    msr cptr_el2, xzr
    isb
skip_cptr_el2:

    adrp x0, __boot_stack_top
    add x0, x0, :lo12:__boot_stack_top
    mov sp, x0

    adrp x1, __bss_start
    add x1, x1, :lo12:__bss_start
    adrp x2, __bss_end
    add x2, x2, :lo12:__bss_end
zero_bss:
    cmp x1, x2
    b.ge zero_bss_done
    str xzr, [x1], #8
    b zero_bss
zero_bss_done:

    mov x0, x20
    bl hvMain

hv_hang:
    wfe
    b hv_hang

hv_no_el2_hang:
    wfe
    b hv_no_el2_hang

.section .bss
.balign 8
.global hv_smp_wake_flag
hv_smp_wake_flag:
    .quad 0

// --- EL2 exception vector table (VBAR_EL2) ---
//
// Same 16-slot/0x80-stride/four-group layout as kernel-lib's own
// VBAR_EL1 table (mod.rs, "vector_table:"); the "lower EL, AArch64" group
// is what matters here, since the guest always runs at EL1 and every HVC
// call, GICD/GICR stage-2 fault, and passed-through IRQ arrives through
// those four slots. "Current EL" handles the hv's own faults; "lower EL,
// AArch32" is unused - the guest is aarch64-only.
.section .text.exceptions
.balign 0x800
.global vector_table_el2
vector_table_el2:
    // Current EL, SP0
    b sync_trampoline; .balign 0x80
    b irq_trampoline; .balign 0x80
    b fiq_trampoline; .balign 0x80
    b serror_trampoline; .balign 0x80
    // Current EL, SPx
    b sync_trampoline; .balign 0x80
    b irq_trampoline; .balign 0x80
    b fiq_trampoline; .balign 0x80
    b serror_trampoline; .balign 0x80
    // Lower EL, AArch64 - the guest
    b sync_trampoline; .balign 0x80
    b irq_trampoline; .balign 0x80
    b fiq_trampoline; .balign 0x80
    b serror_trampoline; .balign 0x80
    // Lower EL, AArch32 (unused)
    b unexpected_trampoline; .balign 0x80
    b unexpected_trampoline; .balign 0x80
    b unexpected_trampoline; .balign 0x80
    b unexpected_trampoline; .balign 0x80

// --- EL1 catch vector table (installed as the guest's VBAR_EL1) ---
//
// The guest runs with its own MMU off under an identity stage-2 map, so it
// can fetch and execute this hypervisor's code at its physical address.
// Installing this as VBAR_EL1 closes the last blind spot in guest
// diagnostics: any exception NOT routed to EL2 by HCR_EL2 - undefined
// instruction, SVC, stage-1 fault once the guest enables its MMU,
// misaligned SP - is delivered to EL1 instead, and with the boot ROM's
// VBAR_EL1 (observed: 0) those vanish into whatever sits at address 0 with
// no report of any kind. That is indistinguishable from "the guest never
// ran", which is exactly the ambiguity this resolves.
//
// Each slot identifies itself to EL2 with a 0xE1CA-tagged function ID
// carrying its slot index, and passes ESR_EL1/ELR_EL1 in x1/x2 so the
// unhandled-HVC path reports the cause and faulting PC. After EL2 returns,
// the slot parks rather than retrying, so a fault storm can't scroll the
// report off the panel.
.balign 0x800
.global el1_catch_vectors
el1_catch_vectors:
.macro EL1_CATCH idx
    mrs x1, esr_el1
    mrs x2, elr_el1
    movz x0, #\idx
    movk x0, #0xe1ca, lsl #16
    hvc #0
    b .
    .balign 0x80
.endm
    // Current EL, SP0
    EL1_CATCH 0
    EL1_CATCH 1
    EL1_CATCH 2
    EL1_CATCH 3
    // Current EL, SPx
    EL1_CATCH 4
    EL1_CATCH 5
    EL1_CATCH 6
    EL1_CATCH 7
    // Lower EL (EL0), AArch64
    EL1_CATCH 8
    EL1_CATCH 9
    EL1_CATCH 10
    EL1_CATCH 11
    // Lower EL, AArch32
    EL1_CATCH 12
    EL1_CATCH 13
    EL1_CATCH 14
    EL1_CATCH 15

// Frame is 816 bytes: 30 GPRs (x0..x29) + x30 + sp_el0/elr_el2 +
// spsr_el2/esr_el2 + far_el2/hpfar_el2 + 8 bytes padding (304 total),
// followed by q0..q31 (512 bytes). Must match context.rs's `Frame`
// exactly - see that file's doc comment for the field-by-field rationale
// (elr_el2/spsr_el2/esr_el2/far_el2 instead of kernel-lib's _el1
// register bank, plus hpfar_el2 with no EL1-frame equivalent at all).
sync_trampoline:
    sub sp, sp, #816
    stp x0, x1, [sp, #0]
    stp x2, x3, [sp, #16]
    stp x4, x5, [sp, #32]
    stp x6, x7, [sp, #48]
    stp x8, x9, [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30, [sp, #240]
    mrs x1, sp_el0
    mrs x2, elr_el2
    stp x1, x2, [sp, #248]
    mrs x1, spsr_el2
    mrs x2, esr_el2
    stp x1, x2, [sp, #264]
    mrs x1, far_el2
    mrs x2, hpfar_el2
    stp x1, x2, [sp, #280]
    stp q0, q1, [sp, #304]
    stp q2, q3, [sp, #336]
    stp q4, q5, [sp, #368]
    stp q6, q7, [sp, #400]
    stp q8, q9, [sp, #432]
    stp q10, q11, [sp, #464]
    stp q12, q13, [sp, #496]
    stp q14, q15, [sp, #528]
    stp q16, q17, [sp, #560]
    stp q18, q19, [sp, #592]
    stp q20, q21, [sp, #624]
    stp q22, q23, [sp, #656]
    stp q24, q25, [sp, #688]
    stp q26, q27, [sp, #720]
    stp q28, q29, [sp, #752]
    stp q30, q31, [sp, #784]
    mov x0, sp
    bl handleSyncExceptionEl2
    b restore_and_eret

irq_trampoline:
    sub sp, sp, #816
    stp x0, x1, [sp, #0]
    stp x2, x3, [sp, #16]
    stp x4, x5, [sp, #32]
    stp x6, x7, [sp, #48]
    stp x8, x9, [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30, [sp, #240]
    mrs x1, sp_el0
    mrs x2, elr_el2
    stp x1, x2, [sp, #248]
    mrs x1, spsr_el2
    mrs x2, esr_el2
    stp x1, x2, [sp, #264]
    mrs x1, far_el2
    mrs x2, hpfar_el2
    stp x1, x2, [sp, #280]
    stp q0, q1, [sp, #304]
    stp q2, q3, [sp, #336]
    stp q4, q5, [sp, #368]
    stp q6, q7, [sp, #400]
    stp q8, q9, [sp, #432]
    stp q10, q11, [sp, #464]
    stp q12, q13, [sp, #496]
    stp q14, q15, [sp, #528]
    stp q16, q17, [sp, #560]
    stp q18, q19, [sp, #592]
    stp q20, q21, [sp, #624]
    stp q22, q23, [sp, #656]
    stp q24, q25, [sp, #688]
    stp q26, q27, [sp, #720]
    stp q28, q29, [sp, #752]
    stp q30, q31, [sp, #784]
    mov x0, sp
    bl handleIrqExceptionEl2
    b restore_and_eret

fiq_trampoline:
    sub sp, sp, #816
    stp x0, x1, [sp, #0]
    str x30, [sp, #240]
    mov x0, sp
    bl handleFiqExceptionEl2
    add sp, sp, #816
    eret

serror_trampoline:
    sub sp, sp, #816
    stp x0, x1, [sp, #0]
    str x30, [sp, #240]
    mov x0, sp
    bl handleSErrorExceptionEl2
    add sp, sp, #816
    eret

unexpected_trampoline:
    sub sp, sp, #816
    stp x0, x1, [sp, #0]
    str x30, [sp, #240]
    mov x0, sp
    bl handleUnexpectedExceptionEl2
    add sp, sp, #816
    eret

restore_and_eret:
    ldp q30, q31, [sp, #784]
    ldp q28, q29, [sp, #752]
    ldp q26, q27, [sp, #720]
    ldp q24, q25, [sp, #688]
    ldp q22, q23, [sp, #656]
    ldp q20, q21, [sp, #624]
    ldp q18, q19, [sp, #592]
    ldp q16, q17, [sp, #560]
    ldp q14, q15, [sp, #528]
    ldp q12, q13, [sp, #496]
    ldp q10, q11, [sp, #464]
    ldp q8, q9, [sp, #432]
    ldp q6, q7, [sp, #400]
    ldp q4, q5, [sp, #368]
    ldp q2, q3, [sp, #336]
    ldp q0, q1, [sp, #304]
    ldp x1, x2, [sp, #248]
    msr sp_el0, x1
    msr elr_el2, x2
    ldr x1, [sp, #264]
    msr spsr_el2, x1
    ldr x30, [sp, #240]
    ldp x28, x29, [sp, #224]
    ldp x26, x27, [sp, #208]
    ldp x24, x25, [sp, #192]
    ldp x22, x23, [sp, #176]
    ldp x20, x21, [sp, #160]
    ldp x18, x19, [sp, #144]
    ldp x16, x17, [sp, #128]
    ldp x14, x15, [sp, #112]
    ldp x12, x13, [sp, #96]
    ldp x10, x11, [sp, #80]
    ldp x8, x9, [sp, #64]
    ldp x6, x7, [sp, #48]
    ldp x4, x5, [sp, #32]
    ldp x2, x3, [sp, #16]
    ldp x0, x1, [sp, #0]
    add sp, sp, #816
    eret
"#
));
