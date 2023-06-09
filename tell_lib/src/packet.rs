use serde::{Serialize, Deserialize};
use crate::{id::Id, header::PacketHeader};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PacketType {
    Client(ClientPacket),
    Server(ServerPacket),
    Heartbeat
}

impl PacketType {
    pub fn client(self) -> Option<ClientPacket> {
        match self {
            PacketType::Client(packet) => Some(packet),
            _ => None
        }
    }

    pub fn server(self) -> Option<ServerPacket> {
        match self {
            PacketType::Server(packet) => Some(packet),
            _ => None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TargetMode {
    Broadcast,
    Multicast(Vec<Id>),
    Unicast(Id)
}

// #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// pub enum RequestType {
//     Peers
// }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientPacket {
    Connect,
    Disconnect,
    Message(TargetMode, String),
    RequestPeers
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DisconnectReason {
    Manual,
    Timeout
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerPacket {
    PeerConnected(Id),
    PeerDisconnected(Id, DisconnectReason),
    PeerTimedOut(Id),
    Message {
        source: Id,
        target_mode: TargetMode,
        text: String
    },
    RequestReply(Vec<Id>)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Packet {
    pub header: PacketHeader,
    pub payload: PacketType
}

impl Packet {
    pub fn new(source: Id, payload: PacketType) -> Self {
        Self {
            header: PacketHeader::new(source), payload
        }
    }

    pub fn client(source: Id, payload: ClientPacket) -> Self {
        Self::new(source, PacketType::Client(payload))
    }

    pub fn server(source: Id, payload: ServerPacket) -> Self {
        Self::new(source, PacketType::Server(payload))
    }

    pub fn header(&self) -> &PacketHeader {
        &self.header
    }

    pub fn payload(&self) -> &PacketType {
        &self.payload
    }
}
