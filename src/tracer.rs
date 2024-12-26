use crate::{
    proxy_server::{self, PROXY_PORT},
    syscall_util::{self, get_syscall_arg, get_syscall_num},
};
use nix::{
    libc::{self, sockaddr_in},
    sys::{
        ptrace::{self, Options},
        signal::Signal,
        wait::{wait, waitpid, WaitStatus},
    },
    unistd::Pid,
};
use std::{
    collections::HashMap,
    mem,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

const SOCKADDR_LEN: usize = mem::size_of::<sockaddr_in>();

pub struct Tracer {
    parent_pid: Pid,
    sender: mpsc::Sender<FakeIpOpt>,
}

#[derive(Debug)]
pub enum FakeIpOpt {
    Set {
        real: SocketAddr,
        fake: SocketAddr,
        resp: oneshot::Sender<bool>,
    },
}

impl Tracer {
    pub fn new(parent_pid: Pid, sender: mpsc::Sender<FakeIpOpt>) -> Tracer {
        Tracer { parent_pid, sender }
    }

    pub fn trace(&self, cancel_token: CancellationToken) {
        let mut proc_info: HashMap<Pid, ProcInfo> = HashMap::new();
        let options = Options::PTRACE_O_TRACESYSGOOD
            | Options::PTRACE_O_TRACECLONE
            | Options::PTRACE_O_TRACEFORK
            | Options::PTRACE_O_TRACEVFORK
            | Options::PTRACE_O_TRACEEXEC;
        // the child will stop when ready to be traced
        waitpid(self.parent_pid, None).unwrap();
        log::info!("parent pid:{}", self.parent_pid);
        ptrace::setoptions(self.parent_pid, options).unwrap();
        ptrace::syscall(self.parent_pid, None).unwrap();
        let mut fakeip_gen = FakeIpGen::new();
        loop {
            let mut sig = None;
            let status = wait().unwrap();
            let pid = status.pid().unwrap();
            match status {
                WaitStatus::PtraceSyscall(_) => {
                    syscall_handle(pid, &mut proc_info, &self.sender, &mut fakeip_gen);
                    ptrace::syscall(pid, sig).unwrap();
                }
                WaitStatus::Exited(pid, _) => {
                    if pid == self.parent_pid {
                        break;
                    }
                    // ptrace::syscall(self.parent_pid, sig).unwrap();
                }
                WaitStatus::Signaled(_, _, _) => {
                    if pid == self.parent_pid {
                        break;
                    }
                    // ptrace::syscall(self.parent_pid, sig).unwrap();
                }
                WaitStatus::Stopped(_, signal) => {
                    // signal injection
                    if signal != Signal::SIGSTOP {
                        sig = Some(signal);
                    }
                    ptrace::syscall(pid, sig).unwrap();
                }
                WaitStatus::PtraceEvent(_, _, _) => {
                    ptrace::syscall(pid, sig).unwrap();
                }
                _ => (),
            }
        }
        cancel_token.cancel();
        log::info!("command finish,sent cancel token");
    }
}

struct ProcInfo {
    is_entry: bool,
}

fn syscall_handle(
    pid: Pid,
    proc_info: &mut HashMap<Pid, ProcInfo>,
    sender: &mpsc::Sender<FakeIpOpt>,
    fakeip_gen: &mut FakeIpGen,
) {
    match proc_info.get_mut(&pid) {
        None => {
            proc_info.insert(pid, ProcInfo { is_entry: true });
            entry(pid, sender, fakeip_gen);
        }
        Some(proc_info) => {
            if proc_info.is_entry {
                exit(pid);
                proc_info.is_entry = false;
            } else {
                entry(pid, sender, fakeip_gen);
                proc_info.is_entry = true;
            }
        }
    }
}

//
fn entry(pid: Pid, sender: &mpsc::Sender<FakeIpOpt>, fakeip_gen: &mut FakeIpGen) {
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
        let fakeip = fakeip_gen.generate_fakeip();
        syscall_util::write_data(
            pid,
            sockaddr_ptr as *mut libc::c_void,
            &get_proxy_sockaddr_bytes_array(fakeip.octets()),
        );
        let (resp_tx, resp_rx) = oneshot::channel();
        let cmd = FakeIpOpt::Set {
            real: SocketAddr::new(IpAddr::V4(addr), port),
            fake: SocketAddr::new(IpAddr::V4(fakeip), PROXY_PORT),
            resp: resp_tx,
        };
        sender.blocking_send(cmd).unwrap();
        resp_rx.blocking_recv().unwrap();
    }
}

fn exit(_: Pid) {}

fn get_proxy_sockaddr_bytes_array(addr: [u8; 4]) -> [u8; SOCKADDR_LEN] {
    let proxy_addr: sockaddr_in = sockaddr_in {
        sin_family: libc::AF_INET as u16,
        sin_port: u16::from_be(proxy_server::PROXY_PORT),
        sin_addr: libc::in_addr {
            s_addr: u32::from_ne_bytes(addr),
        },
        sin_zero: [0; 8],
    };
    *unsafe { mem::transmute::<&sockaddr_in, &[u8; SOCKADDR_LEN]>(&proxy_addr) }
}

struct FakeIpGen {
    flag: u32,
}

impl FakeIpGen {
    pub fn new() -> FakeIpGen {
        FakeIpGen { flag: 0x7f000000 }
    }
    // 最大支持 16^4 = 65536 个连接
    pub fn generate_fakeip(&mut self) -> Ipv4Addr {
        if self.flag == 0x7f00fffe {
            //127.00.255.254
            self.flag = 0x7f000000;
        }
        self.flag += 1;
        Ipv4Addr::from_bits(self.flag)
    }
}
