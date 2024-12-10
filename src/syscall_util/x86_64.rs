use nix::libc::user_regs_struct;
pub fn get_syscall_num(regs: &user_regs_struct) -> u64 {
    regs.orig_rax
}

pub fn get_syscall_arg(regs: &user_regs_struct) -> Argument {
    Argument{
        arg1: regs.rdi,
        arg2: regs.rsi,
        arg3: regs.rdx,
        arg4: regs.r10,
    }
}

pub struct Argument {
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
    pub arg4: u64,
}
