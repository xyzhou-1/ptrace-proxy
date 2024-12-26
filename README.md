# What is this
This is a simple tool to redirect tcp traffic from any application to http proxy.
# Usage
`[RUST_LOG=info] cargo run <command> [args]`

>note:only support ipv4 tcp

# How it works
use ptrace to trace the process and redirect tcp packet to proxy port by modify `connect` syscall arguments.

# Acknowledgements and References
[graftcp](https://github.com/hmgle/graftcp)

# todo
- [ ] not hard code proxy port 10810
- [ ] support ipv6
- [ ] use ebpf instead of ptrace to improve performance

# My notes from manpage

ptrace:
>The call
>
>ptrace(PTRACE_SETOPTIONS, pid, 0, PTRACE_O_flags);
>
>affects one tracee. The tracee's current flags are replaced.
>Flags are inherited by new tracees created and "auto-attached" via active PTRACE_O_TRACEFORK, PTRACE_O_TRACEVFORK, or PTRACE_O_TRACECLONE options.

>Syscall-enter-stop and syscall-exit-stop are indistinguishable from each other by the tracer.
>The tracer needs to keep track of the sequence of ptrace-stops in order to not misinterpret syscall-enter-stop as syscall-exit-stop or vice versa.
>In general, a syscall-enter-stop is always followed by syscall-exit-stop, PTRACE_EVENT stop, or the tracee's death; no other kinds of ptrace-stop can occur in between.
>However, note that seccomp stops (see below) can cause syscall-exit-stops, without preceding syscall-entry-stops.
>If seccomp is in use, care needs to be taken not to misinterpret such stops as syscall-entry-stops.
>
>If after syscall-enter-stop, the tracer uses a restarting command other than PTRACE_SYSCALL, syscall-exit-stop is not generated.