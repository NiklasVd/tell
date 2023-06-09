use std::{net::SocketAddr, collections::HashSet};
use log::{warn, info, error};
use crate::{id::Id, err::{TResult, TellErr, LibErr}, packet::{ClientPacket, PacketType, TargetMode, Packet, ServerPacket}, event::UdpAdapterEvent, net::conn::{Connection, UdpConnection}};
use super::adapter::{UdpAdapter, AdapterConfig, SendMode};

pub struct Client {
    id: Id,
    peers: HashSet<Id>,
    chat_log: Vec<(Id, String)>,
    remote_addr: Option<SocketAddr>, // Pending connection?
    adapter: UdpAdapter
}

impl Client {
    pub fn new(id: Id, port: u16) -> TResult<Self> {
        let adapter = UdpAdapter::new(id.clone(), AdapterConfig {
            port, max_conns: 1 // Only peer: server.
        })?;
        Ok(Client {
            id, peers: HashSet::new(), chat_log: vec![], remote_addr: None, adapter
        })
    }

    pub fn connect(&mut self, remote_addr: SocketAddr) -> TResult {
        if let Some(addr) = self.remote_addr {
            Err(TellErr::Lib(LibErr::PeerAlreadyConnected(addr)))
        } else {
            info!("Connecting with {remote_addr}...");
            self.adapter.send_command(SendMode::Unicast(remote_addr), 
            PacketType::Client(ClientPacket::Connect))?;
            self.adapter.shared_state.lock().unwrap().add_conn(UdpConnection::outgoing(remote_addr))?;
            self.remote_addr = Some(remote_addr);
            Ok(())
        }
    }

    pub fn dispose(self) -> TResult {
        match self.adapter.thread_handle.join() {
            Ok(res) => Ok(res?),
            Err(e) => Err(e.into())
        }
    }

    pub fn shutdown(self) -> TResult {
        // self.adapter.shared_state.lock().unwrap().shutdown();
        self.adapter.shared_state.lock().unwrap().shutdown();
        self.dispose()
    }

    pub fn reset_connection(&mut self) -> TResult {
        if let Some(addr) = self.remote_addr.clone() {
            self.remote_addr = None;
            info!("Reset connection with {addr}.");
            if let Some(conn) = self.adapter.shared_state.lock().unwrap()
                .remove_conn(addr) {
            }
            Ok(())
        } else {
            Err(TellErr::Lib(LibErr::NotConnected))
        }
    }

    pub fn disconnect(&mut self) -> TResult {
        if let Some(addr) = self.remote_addr.clone() {
            info!("Disconnecting from {addr}.");
            self.send_packet(ClientPacket::Disconnect)?;
        } else {
            return Err(TellErr::Lib(LibErr::NotConnected))
        }
        self.reset_connection()
    }

    // Is a connection attempt undergoing right now?
    pub fn connecting(&self) -> bool {
        self.remote_addr.is_some()
    }

    // Has a connection been established?
    pub fn connected(&self) -> Option<Id> {
        if let Some(addr) = self.remote_addr.as_ref() {
            if let Some(conn) = self.adapter.shared_state.lock().unwrap().conns.get(&addr) {
                return conn.id().cloned()
            }
        }
        None
    }

    pub fn message(&self, target_mode: TargetMode, text: String) -> TResult {
        self.send_packet(ClientPacket::Message(target_mode, text))
    }

    pub fn print_metrics(&self) {
        self.adapter.shared_state.lock().unwrap().conns.values().for_each(|conn| {
            info!("{:?}{:?} metrics: Sent {:?}, Recv {:?}",
                conn.addr(), conn.id(), conn.send_metrics(), conn.recv_metrics());
        })
    }

    pub fn poll(&mut self) -> TResult {
        //let mut _shared_state = self.adapter.shared_state.lock().unwrap();
        Ok(for ev in self.adapter.flush_events() {
            if !self.connecting() {
                return Err(TellErr::Lib(LibErr::NotConnected))
            } else {
                info!("Client event: {:?}.", ev);
                self.handle_event(ev)?;
            }
        })
    }

    fn handle_event(&mut self, ev: UdpAdapterEvent) -> TResult {
        match ev {
            UdpAdapterEvent::PeerConnect(addr, packet) => {
                // This peer cannot be connected to. Abort.
                Err(TellErr::Lib(LibErr::InvalidPacketType(format!("Client received invalid connection packet from {addr}: {:?}", packet))))
            },
            UdpAdapterEvent::PeerDisconnect(addr, id, reason) => {
                warn!("[Disconnect] Server {:?}{addr} disconnected. Reason: {:?}.", id, reason);
                if let Some(conn) = self.adapter.shared_state.lock()
                    .unwrap().remove_conn(addr) {
                    info!("Server ({:?}) metrics: {:?}, {:?}.",
                        conn.conn_state(), conn.send_metrics(), conn.recv_metrics());
                } else {
                    info!("Received invalid disconnect packet due to missing connection handle: {:?}{addr}, reason = {:?}", id, reason)
                }
                Ok(())
            },
            UdpAdapterEvent::Payload(addr, packet) => {
                let Packet {
                    header, payload
                } = packet;
                if let Some(server_packet) = payload.server() {
                    // Connection is already established
                    if let Some(id) = self.connected() {
                        self.handle_payload(addr, id, server_packet)
                    } else {
                        self.handle_connect_event(addr, header.source().clone(), server_packet)
                    }
                } else {
                    Err(TellErr::Lib(LibErr::InvalidPacketType(format!("Expected server type packet"))))
                }
            }
        }
    }

    fn handle_payload(&mut self, addr: SocketAddr, id: Id, packet: ServerPacket) -> TResult {
        match packet {
            // ServerPacket::PeerConnected(id) => {
            //     info!("New peer connected: {:?}.", id);

            //     Ok(())
            // },
            // ServerPacket::PeerDisconnected(id, reason) => {
            //     Ok(())
            // },
            ServerPacket::Message { source, target_mode, text } => {
                let target = match target_mode {
                    TargetMode::Broadcast => "broadcasted".to_owned(),
                    TargetMode::Multicast(ids) => format!("wrote to {:?}", ids),
                    TargetMode::Unicast(id) => {
                        if self.id  != id {
                            info!("Oops. Personal message to {:?} was eavesdropped by you.", id);
                        }
                        "whispered to you".to_owned()
                    }
                };
                info!("[Message] {:?} {}: {text}.", source, target);
                self.chat_log.push((source, text));
                Ok(())
            },
            ServerPacket::RequestReply(ids) => {
                info!("Server sent peer list:");
                for id in ids.into_iter() {
                    println!("{:?}", id);
                }
                Ok(())
            },
            p @ _ => Err(TellErr::Lib(LibErr::InvalidPacketType(
                format!("Expected server request or message packet: Recv: {:?}.", p))))

        }
    }

    fn handle_connect_event(&mut self, addr: SocketAddr, source_id: Id, packet: ServerPacket) -> TResult {
        match packet {
            ServerPacket::PeerConnected(id) => {
                if self.id == id {
                    info!("Server accepted connection!");
                    self.adapter.shared_state.lock().unwrap().conns
                        .get_mut(&addr).unwrap().connect(source_id)
                } else {
                    info!("Peer {:?} connected.", id);
                    self.peers.insert(id);
                    Ok(())
                }
            },
            ServerPacket::PeerDisconnected(id, reason) => {
                if self.id == source_id {
                    error!("Server {:?}{addr} rejected connection with us.", source_id);
                    self.reset_connection()
                } else {
                    info!("Peer {:?} disconnected. Reason: {:?}.", id, reason);
                    self.peers.remove(&id);
                    Ok(())
                }
            },
            p @ _ => Err(TellErr::Lib(LibErr::InvalidPacketType(format!("Expected peer connected packet from server. Recv: {:?}.", p))))

        }
    }

    fn send_packet(&self, packet: ClientPacket) -> TResult {
        if let Some(addr) = self.remote_addr.as_ref() {
            self.adapter.send_command(SendMode::Unicast(*addr), 
            PacketType::Client(packet))
        } else {
            Err(TellErr::Lib(LibErr::NotConnected))
        }
    }
}