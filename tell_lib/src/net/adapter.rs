use std::{sync::{Arc, atomic::AtomicBool, Mutex, MutexGuard}, net::{UdpSocket, SocketAddr, SocketAddrV4, Ipv4Addr}, thread::{JoinHandle, self}, io::ErrorKind};
use crossbeam_channel::{Receiver, Sender, unbounded};
use log::{info, warn, error};
use crate::{packet::{Packet, PacketType, DisconnectReason}, event::UdpAdapterEvent, err::TResult, id::Id, builder::{PacketBuilder, PacketReader}};
use super::{shared_state::UdpSharedState, conn::Connection};

pub const UDP_READ_BUF_SIZE: usize = 508;
pub const UDP_HEARTBEAT_INTERVAL: f32 = 1.25;
pub const UDP_HEARTBEAT_INTERVAL_GRACE: f32 = UDP_HEARTBEAT_INTERVAL * 3.;

pub type AMx<T> = Arc<Mutex<T>>;
pub type Sx<T> = Sender<T>;
pub type Rx<T> = Receiver<T>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterConfig {
    pub port: u16,
    pub max_conns: u16
}

#[derive(Debug, Clone, PartialEq)]
pub enum SendMode {
    Broadcast,
    Multicast(Vec<SocketAddr>),
    Unicast(SocketAddr)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SendCommand(SendMode, PacketType);

#[derive(Clone)]
struct UdpAdapterParams {
    shared_state: AMx<UdpSharedState>,
    send_handle: Rx<SendCommand>,
    event_queue: Sx<UdpAdapterEvent>,
    builder: PacketBuilder,
    reader: PacketReader
}

pub struct UdpAdapter {
    pub shared_state: AMx<UdpSharedState>,
    send_queue: Sx<SendCommand>,
    event_handle: Rx<UdpAdapterEvent>,
    pub thread_handle: JoinHandle<TResult>
}

impl UdpAdapter {
    pub fn new(id: Id, config: AdapterConfig) -> TResult<Self> {
        let sock = UdpSocket::bind(
            SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, config.port))?;
        sock.set_nonblocking(true)?;

        let running = AtomicBool::new(true);
        let shared_state = Arc::new(Mutex::new(
            UdpSharedState::new(sock, running, config)));
        
        let (send_queue, send_handle) = unbounded();
        let (event_queue, event_handle) = unbounded();
        let params = UdpAdapterParams {
            shared_state: shared_state.clone(), send_handle, event_queue,
            builder: PacketBuilder::new(id), reader: PacketReader::new()
        };

        let thread_handle = Self::init_thread(params);
        Ok(UdpAdapter {
            shared_state, send_queue, event_handle, thread_handle
        })
    }

    pub fn send_command(&self, send_mode: SendMode, packet: PacketType) -> TResult {
        Ok(self.send_queue.try_send(SendCommand(send_mode, packet))?)
    }

    pub fn flush_events(&self) -> Vec<UdpAdapterEvent> {
        let mut events = vec![];
        while let Ok(ev) = self.event_handle.try_recv() {
            events.push(ev);
        }
        events
    }

    fn init_thread(params: UdpAdapterParams)
            -> JoinHandle<TResult> {
        let handle = thread::spawn(move || {
            loop {
                let mut _shared_state = params.shared_state.lock().unwrap();
                if let Err(e) = Self::send_packets(params.clone(), &mut _shared_state) {
                    error!("Udp adapter thread (send): {e}.");
                    return Err(e)
                }
                if let Err(e) = Self::recv_packets(params.clone(), &mut _shared_state) {
                    error!("Udp adapter thread (send): {e}.");
                    return Err(e)
                }
                if let Err(e) = Self::maintain_conns(params.clone(), &mut _shared_state) {
                    error!("Udp adapter thread (send): {e}.");
                    return Err(e)
                }

                if !_shared_state.running() {
                    info!("Udp adaper thread stopped running");
                    return Ok(())
                }
            }
        });
        handle
    }

    fn send_packets(params: UdpAdapterParams, _shared_state: &mut MutexGuard<'_, UdpSharedState>) -> TResult {
        while let Ok(SendCommand(send_mode, packet)) = params.send_handle.try_recv() {
            let addrs = match send_mode {
                SendMode::Broadcast => _shared_state.conn_addrs(),
                SendMode::Multicast(addrs) => addrs,
                SendMode::Unicast(addr) => vec![addr],
            };
            Self::send_packet(params.clone(), _shared_state, addrs, packet)?;
        }
        Ok(())
    }

    fn send_packet(params: UdpAdapterParams, _shared_state: &mut MutexGuard<'_, UdpSharedState>, addrs: Vec<SocketAddr>, packet: PacketType) -> TResult {
        let bytes = params.builder.serialize(packet)?;
        for addr in addrs.into_iter() {
            if let Some(conn) = _shared_state.conns.get_mut(&addr) {
                conn.send(bytes.len());
            }
            _shared_state.sock.send_to(&bytes, addr)?;
        }
        Ok(())
    }

    fn recv_packets(params: UdpAdapterParams, _shared_state: &mut MutexGuard<'_, UdpSharedState>) -> TResult {
        let mut buf = vec![0u8; UDP_READ_BUF_SIZE * 2];
        match _shared_state.sock.recv_from(&mut buf) {
            Ok((size, addr)) => {
                // Zero bytes an issue?
                let mut bytes = buf[0..size].to_vec();
                let packet = params.reader.deserialize(&mut bytes)?;
                //std::mem::drop(_shared_state);
                Self::recv_packet(params.clone(), _shared_state, addr, size, packet)
            },
            // Recv buffer empty
            Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(()),
            Err(e) => Err(e.into())
        }
    }

    fn recv_packet(params: UdpAdapterParams, _shared_state: &mut MutexGuard<'_, UdpSharedState>, addr: SocketAddr, size: usize, packet: Packet) -> TResult  {
        if let Some(conn) = _shared_state.conns.get_mut(&addr) {
            conn.recv(size);
            Ok(match packet.payload() {
                PacketType::Heartbeat => (), // No need to forward this
                _ => {
                    info!("[Recv] {size}b from {:?}{addr}: {:?}.", packet.header().source(), packet.payload());
                    params.event_queue.try_send(UdpAdapterEvent::Payload(addr, packet))?
                },
            })
        } else { // New connection?
            Ok(match packet.payload {
                PacketType::Heartbeat => (),
                _ => {
                    info!("[Recv] [New] {size}b from {:?}{addr}: {:?}", packet.header().source(), packet.payload());
                    params.event_queue.try_send(UdpAdapterEvent::PeerConnect(addr, packet))?
                }
            })
        }
    }

    fn maintain_conns(params: UdpAdapterParams, _shared_state: &mut MutexGuard<'_, UdpSharedState>) -> TResult {
        let mut notify_addrs = vec![];
        // Collect all connections with no outgoing traffic for long.
        // Also, shoot timeout events for all idle connections.
        // (Higher level logic, i.e., server and client can decide
        // what to do; disconnect/reconnect etc.)
        for conn in _shared_state.conns.values_mut() {
            // Does connection state matter? Curr opinion: No, otherwise idle connections might get forgotten.
            if conn.send_metrics().last_transfer.elapsed().as_secs_f32() >= UDP_HEARTBEAT_INTERVAL {
                notify_addrs.push(conn.addr());
            }
            if conn.recv_metrics().last_transfer.elapsed().as_secs_f32() >= UDP_HEARTBEAT_INTERVAL_GRACE * 1.25 {
                params.event_queue.try_send(
                    UdpAdapterEvent::PeerDisconnect(
                            conn.addr(), conn.id().cloned(), DisconnectReason::Timeout))?;
            }
        }
        // Send out heartbeats
        Self::send_packet(params.clone(), _shared_state, notify_addrs, PacketType::Heartbeat)
    }
}
