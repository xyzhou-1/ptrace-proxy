use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::tracer::FakeIpOpt;

type Db = Arc<Mutex<HashMap<SocketAddr, SocketAddr>>>;

pub struct ProxyServer {
    db: Db,
}

pub const PROXY_PORT: u16 = 10810;
impl ProxyServer {
    pub fn new() -> ProxyServer {
        let db = Arc::new(Mutex::new(HashMap::new()));
        ProxyServer { db }
    }

    pub async fn run(&self, rx: mpsc::Receiver<FakeIpOpt>, cancel_token: CancellationToken) {
        let db = Arc::clone(&self.db);
        {
            let cancel_token = cancel_token.clone();
            tokio::spawn(async move {
                tokio::select! {
                    _ = manage_db(rx, db)=>{}
                    _ = cancel_token.cancelled() =>{log::info!("mange_db task cancelled")},
                }
            });
        }
        let listener = TcpListener::bind(format!("0.0.0.0:{PROXY_PORT}"))
            .await
            .unwrap();
        log::info!("proxy listen on {}", PROXY_PORT);
        tokio::select! {
                _= async{
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let db = Arc::clone(&self.db);
                tokio::spawn(async move {
                    process(stream, db).await;
                });
            }
        } => {}
        _ = cancel_token.cancelled() => {log::info!{"listener closed"}}
        }
        log::info!("proxy closed");
    }
}
async fn manage_db(mut rx: mpsc::Receiver<FakeIpOpt>, db: Db) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            FakeIpOpt::Set {
                real: val,
                fake: key,
                resp,
            } => {
                db.lock().unwrap().insert(key, val);
                resp.send(true).unwrap();
            } // FakeIpOpt::Delete { key, resp } => {
              //     db.lock().unwrap().remove(&key);
              //     resp.send(true).unwrap();
              // }
        }
    }
}

async fn process(mut stream: TcpStream, db: Db) {
    let fakeip = &stream.local_addr().unwrap().ip();
    let realip;
    {
        let db = db.lock().unwrap();
        realip = *db.get(&SocketAddr::new(*fakeip, PROXY_PORT)).unwrap();
    }
    let mut proxy_stream = TcpStream::connect("127.0.0.1:10809").await.unwrap();
    let request = format!("CONNECT {} HTTP/1.1\nHost: {}\n\n", realip, realip.ip());
    proxy_stream.write_all(request.as_bytes()).await.unwrap();
    let mut res: Vec<u8> = vec![0; 200];
    let _ = proxy_stream.read(&mut res).await;
    let res = String::from_utf8(res).unwrap();
    if res.contains("200") {
        match io::copy_bidirectional(&mut stream, &mut proxy_stream).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("{realip} {e}");
            }
        };
    }
}
