//! A TLS listener for axum: TCP connections are accepted and their handshakes run in tasks of
//! their own (with a time limit), so a slow or broken client never holds up the others.
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::server::TlsStream;

pub struct TlsListener {
    ready: mpsc::Receiver<(TlsStream<TcpStream>, SocketAddr)>,
    local: SocketAddr,
}

impl TlsListener {
    pub fn new(tcp: TcpListener, config: Arc<rustls::ServerConfig>) -> std::io::Result<TlsListener> {
        let local = tcp.local_addr()?;
        let acceptor = TlsAcceptor::from(config);
        let (tx, ready) = mpsc::channel(32);
        tokio::spawn(async move {
            loop {
                let Ok((stream, addr)) = tcp.accept().await else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                };
                let (acceptor, tx) = (acceptor.clone(), tx.clone());
                tokio::spawn(async move {
                    if let Ok(Ok(tls)) = tokio::time::timeout(Duration::from_secs(10), acceptor.accept(stream)).await {
                        let _ = tx.send((tls, addr)).await;
                    }
                });
            }
        });
        Ok(TlsListener { ready, local })
    }
}

impl axum::serve::Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.ready.recv().await {
            Some(conn) => conn,
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Ok(self.local)
    }
}
