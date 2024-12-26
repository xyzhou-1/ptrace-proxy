mod command;
mod proxy_server;
mod syscall_util;
mod tracer;

use crate::command::Command;
use std::{env, thread};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracer::Tracer;

fn main() {
    env_logger::init();
    let (tx, rx) = mpsc::channel(32);
    let args: Vec<String> = env::args().collect();
    let command = Command::new(&args);
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();
    let tracer_handle = thread::spawn(move || {
        thread::sleep(std::time::Duration::from_secs(1)); // wait proxy server ready
        let pid = match command.clone() {
            Ok(pid) => pid,
            Err(err) => panic!("{}", err.desc()),
        };
        let tracer = Tracer::new(pid, tx);
        tracer.trace(cancel_token);
    });
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            let proxy = proxy_server::ProxyServer::new();
            proxy.run(rx, cancel_token_clone).await;
        });
    tracer_handle.join().unwrap();
}
