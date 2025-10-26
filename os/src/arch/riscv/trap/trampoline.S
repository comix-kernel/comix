# Trampoline for trap handling
# This page will be mapped at TRAMPOLINE in both kernel and user space

    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    .align 12  # Align to 4KB page boundary

__alltraps:
    # TODO: Save user context and switch to kernel
    # For now, just hang
    j __alltraps

__restore:
    # TODO: Restore user context and return to user
    # For now, just hang
    j __restore

    .align 12  # Ensure the section is exactly one page
