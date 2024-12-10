use nix::{
    errno::Errno,
    sched::{clone, CloneFlags},
    sys::{
        ptrace::traceme,
        signal::{raise, Signal},
    },
    unistd::{execvpe, Pid},
};
use std::{ffi::CString, net::TcpStream, thread::sleep, time::Duration};

const STACK_SIZE: usize = 1024 * 1024;

pub struct Command {
    pub program: CString,
    pub args: Vec<CString>,
}

impl Command {
    pub fn new(env_args: &[String]) -> Command {
        if env_args.len() < 2 {
            panic!("no command provided");
        }
        Command {
            program: CString::new(env_args[1].clone()).expect("cannot parse args"),
            args: env_args[2..]
                .iter()
                .map(|s| CString::new(s.clone()).expect("cannot pars args"))
                .collect(),
        }
    }

    // clone and exec the command,the child process will be stopped at start and be traced
    pub fn clone(&self) -> Result<Pid, Errno> {
        let mut tmp_stack: [u8; STACK_SIZE] = [0; STACK_SIZE];
        let flags = CloneFlags::empty();
        unsafe {
            clone(
                Box::new(|| self.exec()),
                &mut tmp_stack,
                flags,
                Some(Signal::SIGCHLD as i32),
            )
        }
    }

    fn exec(&self) -> isize {
        traceme().unwrap();
        raise(Signal::SIGSTOP).unwrap();
        // get os env
        // let env_vars: Vec<CString> = std::env::vars()
        //     .map(|(k, v)| CString::new(format!("{}={}", k, v)).unwrap())
        //     .collect();
        // match execvpe::<CString, CString>(&self.program, &self.args, &env_vars) {
        //     Ok(_) => 0,
        //     Err(err) => {
        //         log::error!("error: '{}' occurred when execute {:?}", err, self.program);
        //         -1
        //     }
        // }
        let _stream = TcpStream::connect("13.107.21.200:443").unwrap();
        sleep(Duration::from_secs(5));
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;

    #[test]
    fn parse_command_with_one_arg() {
        let command = Command::new(&["ptrace-proxy".to_owned(), "ls".to_owned(), "-a".to_owned()]);
        assert_eq!(command.program.to_str().unwrap(), "ls");
    }

    #[test]
    fn parse_command_with_no_args() {
        let command = Command::new(&["ptrace-proxy".to_owned(), "ls".to_owned()]);
        assert_eq!(command.program.to_str().unwrap(), "ls");
    }
    #[test]
    fn parse_command_with_multiple_args() {
        let command = Command::new(&[
            "ptrace-proxy".to_owned(),
            "ls".to_owned(),
            "-a".to_owned(),
            "-l".to_owned(),
        ]);
        assert_eq!(command.program.to_str().unwrap(), "ls");
    }
    #[test]
    fn panic_when_no_command() {
        let result = panic::catch_unwind(|| {
            let _ = Command::new(&["ptrace-proxy".to_owned()]);
        });
        assert!(result.is_err());
    }
}
