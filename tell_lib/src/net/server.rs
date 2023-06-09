use std::net::SocketAddr;

use log::{warn, error, info};

use crate::{id::Id, err::{TResult, TellErr, LibErr}, event::UdpAdapterEvent, packet::{Packet, PacketType, ClientPacket, ServerPacket, DisconnectReason, TargetMode}, net::conn::{UdpConnection, Connection, ConnectionState}};

use super::adapter::{UdpAdapter, AdapterConfig, SendMode};

pub struct Server {
    id: Id,
    adapter: UdpAdapter
}

impl Server {
    pub fn setup(id: Id, config: AdapterConfig) -> TResult<Server> {
        let adapter = UdpAdapter::new(id.clone(), config)?;
        Ok(Server {
            id, adapter
        })
    }

    pub fn dispose(self) -> TResult {
        match self.adapter.thread_handle.join() {
            Ok(res) => Ok(res?),
            Err(e) => Err(e.into())
        }
    }

    pub fn shutdown(self) -> TResult {
        // Instead of lock
        self.adapter.shared_state.lock().unwrap().shutdown();
        self.dispose()
    }

    pub fn send_packet(&self, send_mode: SendMode, packet: ServerPacket) -> TResult {
        self.adapter.send_command(send_mode, PacketType::Server(packet))
    } 

    pub fn send_broadcast(&self, packet: ServerPacket) -> TResult {
        self.send_packet(SendMode::Broadcast, packet)
    }

    pub fn print_metrics(&self) {
        for conn in self.adapter.shared_state.lock().unwrap().conns.values() {
            info!("{:?}{:?} metrics: Sent {:?}, Recv {:?}",
                conn.addr(), conn.id(), conn.send_metrics(), conn.recv_metrics());
        }
    }

    pub fn poll(&mut self) -> TResult {
        //let _shared_state = self.adapter.shared_state.lock().unwrap();
        Ok(for ev in self.adapter.flush_events().into_iter() {
            info!("Server event: {:?}.", ev);
            self.handle_event(ev)?;
        })
    }

    fn handle_event(&mut self, ev: UdpAdapterEvent) -> TResult {
        match ev {
            UdpAdapterEvent::PeerConnect(addr, packet) => {
                let Packet {
                    header, payload
                } = packet;
                if let Some(client_packet) = payload.client() {
                    self.handle_connect_event(addr,
                        header.source().clone(), client_packet)
                } else {
                    error!("Recv invalid packet type: server/heartbeat.");
                    // TODO: error handling?
                    //Err(TellErr::Lib(LibErr::InvalidPacketType("Expected client type packet".to_owned())))
                    Ok(())
                }
            },
            UdpAdapterEvent::PeerDisconnect(addr, id, reason) => {
                self.handle_disconnect_event(addr, id, reason)
            },
            UdpAdapterEvent::Payload(addr, packet) => {
                let Packet {
                    header, payload
                } = packet;
                if let Some(client_packet) = payload.client() {
                    self.handle_payload_event(addr, header.source().clone(), client_packet)
                } else {
                    Err(TellErr::Lib(LibErr::InvalidPacketType("Expected client type packet".to_owned())))
                }
            }
        }
    }

    fn handle_connect_event(&mut self, addr: SocketAddr, id: Id, packet: ClientPacket) -> TResult {
        match packet {
            ClientPacket::Connect => {
                info!("[Connect] {:?}{addr} connected to the server!", id);
                // UdpConnection::approving immediately sets connection state to established
                self.adapter.shared_state.lock().unwrap().add_conn(
                    UdpConnection::incoming(addr, id.clone()))?;
                self.send_broadcast(ServerPacket::PeerConnected(id))
            },
            p @ _ => Err(TellErr::Lib(
                LibErr::InvalidPacketType(format!("{:?}", p))))
        }
    }

    fn handle_disconnect_event(&mut self, addr: SocketAddr, id: Option<Id>, reason: DisconnectReason) -> TResult {
        if let Some(conn) = self.adapter.shared_state.lock().unwrap()
            .remove_conn(addr) {
            info!("[Disconnect] {:?}{addr} disconnected. Reason: {:?}.", id, reason);
            info!("Disconnected peer ({:?}) metrics: {:?}, {:?}.", conn.conn_state(), conn.send_metrics(), conn.recv_metrics());
            if conn.conn_state() == ConnectionState::Established {
                // If connection wasn't established, other clients might not now ID
                self.send_broadcast(ServerPacket::PeerDisconnected(id.unwrap(), reason))?;
            }
            Ok(())
        } else {
            Err(TellErr::Lib(LibErr::PeerNotConnected(addr)))
        }
    }

    fn handle_payload_event(&mut self, addr: SocketAddr, id: Id, packet: ClientPacket) -> TResult {
        match packet {
            ClientPacket::Disconnect => self.handle_disconnect_event(addr, Some(id), DisconnectReason::Manual),
            ClientPacket::Message(target_mode, text) => {
                let _shared_state = self.adapter.shared_state.lock().unwrap();
                let send_mode = match &target_mode {
                    TargetMode::Broadcast => SendMode::Broadcast,
                    // For multicast, filter out all established connections and look up their addrs
                    TargetMode::Multicast(ids) => SendMode::Multicast(
                        _shared_state.conns.values()
                        .filter_map(|conn| {
                            if let Some(id) = conn.id() {
                                if ids.contains(id) {
                                    return Some(conn.addr())
                                }
                            }
                            None
                        })
                        .collect::<Vec<_>>()),
                    TargetMode::Unicast(id) => todo!(),
                };
                self.send_packet(send_mode, ServerPacket::Message {
                    source: id, target_mode, text
                })
            },
            ClientPacket::RequestPeers => {
                let ids = self.adapter.shared_state
                    .lock().unwrap().conns.values()
                    .filter_map(|conn| {
                        conn.id().cloned()
                }).collect::<Vec<_>>();
                self.send_packet(SendMode::Unicast(addr), ServerPacket::RequestReply(ids))
            },
            p @ _ => Err(TellErr::Lib(
                    LibErr::InvalidPacketType(format!("Expected disconnect/message/request. Recv: {:?}", p))))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{id::Id, net::{adapter::AdapterConfig, client::Client}, packet::TargetMode};
    use super::Server;

    #[test]
    fn connect() {
        simple_logger::init().unwrap();
        let mut server = Server::setup(
            Id::new("Chef".to_owned()).unwrap(), AdapterConfig {
                port: 22089, max_conns: 3
            }).unwrap();
        let mut client = Client::new(
            Id::new("Some dude".to_owned()).unwrap(), 33089).unwrap();
        client.connect(format!("127.0.0.1:22089").parse().unwrap()).unwrap();
        for _ in 0..1000 {
            server.poll().unwrap();
            client.poll().unwrap();
        }
        std::thread::sleep(Duration::from_secs(2));
        server.shutdown().unwrap();
        client.shutdown().unwrap();
    }

    #[test]
    fn message() {
        simple_logger::init().unwrap();
        let mut server = Server::setup(
            Id::new("Chef".to_owned()).unwrap(), AdapterConfig {
                port: 22089, max_conns: 3
            }).unwrap();
        let mut client = Client::new(
            Id::new("Some dude".to_owned()).unwrap(), 33089).unwrap();
        client.connect(format!("127.0.0.1:22089").parse().unwrap()).unwrap();
        for _ in 0..1000 {
            server.poll().unwrap();
            client.message(TargetMode::Broadcast, "Hello world!".to_owned()).unwrap();
            client.poll().unwrap();
        }
        std::thread::sleep(Duration::from_secs(2));
        server.shutdown().unwrap();
        client.shutdown().unwrap();
    }
}
