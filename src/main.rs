mod command;
mod proxy_server;
mod syscall_util;
mod tracer;

use crate::command::Command;
use std::{env, thread};

#[tokio::main]
async fn main() {
    env_logger::init();
    let args: Vec<String> = env::args().collect();
    let command = Command::new(&args);
    let tracer_handle = thread::spawn(move || {
        let pid = match command.clone() {
            Ok(pid) => pid,
            Err(err) => panic!("{}", err.desc()),
        };
        tracer::trace(pid);
    });
    tokio::spawn(async move {
        proxy_server::proxy().await;
    });
    tracer_handle.join().unwrap();
}
