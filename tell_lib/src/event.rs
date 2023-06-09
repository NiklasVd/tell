use std::{net::SocketAddr};
use crate::{packet::{Packet, DisconnectReason}, id::Id};

#[derive(Debug, Clone)]
pub enum UdpAdapterEvent {
    PeerConnect(SocketAddr, Packet),
    PeerDisconnect(SocketAddr, Option<Id>, DisconnectReason),
    Payload(SocketAddr, Packet)
}
