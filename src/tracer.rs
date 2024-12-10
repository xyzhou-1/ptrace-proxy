use crate::syscall_util::{self, get_syscall_arg, get_syscall_num};
use nix::{
    libc::{self, sockaddr_in},
    sys::{
        ptrace::{self, Options},
        signal::Signal,
        wait::{wait, waitpid, WaitStatus},
    },
    unistd::Pid,
};
use std::{collections::HashMap, mem, net::Ipv4Addr};

const PROXY_SOCKADDR: sockaddr_in = sockaddr_in {
    sin_family: libc::AF_INET as u16,
    sin_port: u16::from_be(10809),
    sin_addr: libc::in_addr {
        s_addr: u32::from_ne_bytes([127, 0, 0, 1]),
    },
    sin_zero: [0; 8],
};

const SOCKADDR_LEN: usize = mem::size_of::<sockaddr_in>();

const fn get_proxy_sockaddr_bytes_array() -> [u8; SOCKADDR_LEN] {
    *unsafe { mem::transmute::<&sockaddr_in, &[u8; SOCKADDR_LEN]>(&PROXY_SOCKADDR) }
}

pub fn trace(parent_pid: Pid) {
    let mut proc_info: HashMap<Pid, ProcInfo> = HashMap::new();
    let mut sig = None;
    //
    let options = Options::PTRACE_O_TRACESYSGOOD
        | Options::PTRACE_O_TRACECLONE
        | Options::PTRACE_O_TRACEFORK
        | Options::PTRACE_O_TRACEVFORK
        | Options::PTRACE_O_TRACEEXEC;
    // the child will stop when ready to be traced
    waitpid(parent_pid, None).unwrap();
    ptrace::setoptions(parent_pid, options).unwrap();
    ptrace::syscall(parent_pid, None).unwrap();
    loop {
        let status = wait().unwrap();
        let pid = status.pid().unwrap();
        match status {
            WaitStatus::PtraceSyscall(_) => {
                syscall_handle(pid, &mut proc_info);
            }
            WaitStatus::Exited(pid, _) => {
                if pid == parent_pid {
                    break;
                }
            }
            WaitStatus::Signaled(_, _, _) => {
                if pid == parent_pid {
                    break;
                }
            }
            WaitStatus::Stopped(_, signal) => {
                // signal injection
                if signal != Signal::SIGSTOP {
                    sig = Some(signal);
                }
            }
            _ => (),
        }
        ptrace::syscall(pid, sig).unwrap();
    }
}

struct ProcInfo {
    is_entry: bool,
}

fn syscall_handle(pid: Pid, proc_info: &mut HashMap<Pid, ProcInfo>) {
    match proc_info.get_mut(&pid) {
        None => {
            proc_info.insert(pid, ProcInfo { is_entry: true });
            entry(pid);
        }
        Some(proc_info) => {
            if proc_info.is_entry {
                exit(pid);
                proc_info.is_entry = false;
            } else {
                entry(pid);
                proc_info.is_entry = true;
            }
        }
    }
}

//
fn entry(pid: Pid) {
    let regs = ptrace::getregs(pid).unwrap();
    let syscall_num = get_syscall_num(&regs) as i64;
    if syscall_num == libc::SYS_connect {
        let sockaddr_ptr = get_syscall_arg(&regs).arg2;
        let sockaddr_size = get_syscall_arg(&regs).arg3;
        let sockaddr_raw = syscall_util::read_data(
            pid,
            sockaddr_ptr as *mut libc::c_void,
            sockaddr_size as usize,
        )
        .unwrap();
        let sockaddr = unsafe { *sockaddr_raw.as_ptr().cast::<libc::sockaddr_in>() };
        if sockaddr.sin_family != libc::AF_INET as u16 {
            // now,only support ipv4
            return;
        }
        // https://man.archlinux.org/man/sa_family_t.3type
        // the sin_port and sin_addr members are stored in network byte order.
        let addr = Ipv4Addr::from_bits(sockaddr.sin_addr.s_addr.to_be());
        let port = u16::from_be(sockaddr.sin_port);
        log::info!("pid {} connect to {}:{}", pid, addr, port);
        if addr.is_loopback() {
            return;
        }
        syscall_util::write_data(
            pid,
            sockaddr_ptr as *mut libc::c_void,
            &get_proxy_sockaddr_bytes_array(),
        );
    }
}

fn exit(pid: Pid) {}
