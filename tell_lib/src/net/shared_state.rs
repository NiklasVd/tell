use std::{sync::{atomic::{AtomicBool, Ordering}, Mutex}, net::{UdpSocket, SocketAddr}, collections::HashMap};
use crate::{err::{TResult, TellErr, LibErr}, util::Metrics, id::Id};
use super::{conn::{UdpConnection, Connection}, adapter::AdapterConfig};

pub trait SharedState {
    fn running(&self) -> bool;
    fn shutdown(&self) -> bool; // Set running = false
    fn send_metrics(&self) -> Metrics;
    fn recv_metrics(&self) -> Metrics;
}

pub struct UdpSharedState {
    pub sock: UdpSocket,
    running: AtomicBool,
    pub conns: HashMap<SocketAddr, UdpConnection>,
    pub conn_ids: HashMap<Id, SocketAddr>,
    config: AdapterConfig
}

impl UdpSharedState {
    pub fn new(sock: UdpSocket, running: AtomicBool, config: AdapterConfig) -> Self {
        Self {
            sock, running, conns: HashMap::new(), conn_ids: HashMap::new(), config
        }
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn shutdown(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub fn add_conn(&mut self, conn: UdpConnection) -> TResult {
        if self.conns.contains_key(&conn.addr()) {
            Err(TellErr::Lib(LibErr::PeerAlreadyConnected(conn.addr())))
        } else if self.conns.len() >= self.config.max_conns as usize {
            Err(TellErr::Lib(LibErr::MaxConnectionsReached(self.conns.len())))
        } else {
            self.conns.insert(conn.addr(), conn);
            Ok(())
        }
    }

    pub fn remove_conn(&mut self, addr: SocketAddr) -> Option<UdpConnection> {
        self.conns.remove(&addr)
    }

    pub fn conn_addrs(&self) -> Vec<SocketAddr> {
        self.conns.keys().map(|&addr| addr).collect()
    }
}
