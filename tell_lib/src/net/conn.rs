use std::net::SocketAddr;
use crate::{id::Id, util::Metrics, err::TResult};

pub trait Connection {
    fn addr(&self) -> SocketAddr;
    fn conn_state(&self) -> ConnectionState;
    fn id(&self) -> Option<&Id>;
    fn send_metrics(&self) -> Metrics;
    fn recv_metrics(&self) -> Metrics;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Approving,
    Established
}

pub struct UdpConnection {
    addr: SocketAddr,
    // Mark connection origin? Because, if incoming --> established, it's impossible to
    // see who initiated the connection.
    conn_state: ConnectionState,
    id: Option<Id>,
    send_m: Metrics,
    recv_m: Metrics
}

impl UdpConnection {
    pub fn new(addr: SocketAddr, conn_state: ConnectionState, id: Option<Id>) -> Self {
        Self {
            addr, conn_state, id, send_m: Metrics::new(), recv_m: Metrics::new()
        }
    }

    pub fn outgoing(addr: SocketAddr) -> Self {
        Self::new(addr, ConnectionState::Connecting, None)
    }

    pub fn incoming(addr: SocketAddr, id: Id) -> Self {
        // What about the approving state?
        Self::new(addr, ConnectionState::Established, Some(id))
    }

    pub fn connect(&mut self, id: Id) -> TResult {
        self.conn_state = ConnectionState::Established;
        self.id = Some(id);
        Ok(())
    }

    pub fn approve(&mut self) -> TResult {
        self.conn_state = ConnectionState::Established;
        Ok(())
    }

    pub fn send(&mut self, size: usize) {
        self.send_m.transfer(size)

    }

    pub fn recv(&mut self, size: usize) {
        self.recv_m.transfer(size)
    }
}

impl Connection for UdpConnection {
    fn addr(&self) -> SocketAddr {
        self.addr
    }

    fn conn_state(&self) -> ConnectionState {
        self.conn_state
    }

    fn id(&self) -> Option<&Id> {
        self.id.as_ref()
    }

    fn send_metrics(&self) -> Metrics {
        self.send_m
    }

    fn recv_metrics(&self) -> Metrics {
        self.recv_m
    }
}
