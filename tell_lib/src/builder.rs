use std::io::Cursor;

use rmp_serde::{Serializer, Deserializer};
use serde::{Serialize, Deserialize};
use crate::{id::Id, packet::{PacketType, Packet}, err::TResult};

#[derive(Clone)]
pub struct PacketBuilder {
    id: Id
}

impl PacketBuilder {
    pub fn new(id: Id) -> PacketBuilder {
        PacketBuilder {
            id
        }
    }

    pub fn serialize(&self, packet: PacketType) -> TResult<Vec<u8>> {
        let packet = self.gen_packet(packet);
        let mut buf = vec![];
        let mut ser = Serializer::new(&mut buf);
        packet.serialize(&mut ser)?;
        Ok(buf)
    }

    fn gen_packet(&self, packet: PacketType) -> Packet {
        Packet::new(self.id.clone(), packet)
    }
}

#[derive(Clone)]
pub struct PacketReader {
}

impl PacketReader {
    pub fn new() -> PacketReader {
        PacketReader{}
    }

    pub fn deserialize(&self, buf: &mut Vec<u8>) -> TResult<Packet> {
        let mut reader = Cursor::new(buf);
        let mut de = Deserializer::new(&mut reader);
        Ok(Packet::deserialize(&mut de)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::{id::Id, packet::{PacketType, ServerPacket}, builder::PacketReader};

    use super::PacketBuilder;

    #[test]
    fn serialize() {
        let builder = PacketBuilder::new(Id::new("Bob".to_owned()).unwrap());
        let packet = PacketType::Server(
            ServerPacket::PeerConnected(Id::new("Alice".to_owned()).unwrap()));
        let bytes = builder.serialize(packet).unwrap();
        assert_eq!(bytes.len(), 91);
    }

    #[test]
    fn deserialize() {
        let builder = PacketBuilder::new(Id::new("Bob".to_owned()).unwrap());
        let packet = PacketType::Server(
            ServerPacket::PeerConnected(Id::new("Alice".to_owned()).unwrap()));
        let mut bytes = builder.serialize(packet.clone()).unwrap();
        assert_eq!(bytes.len(), 91);
        let reader = PacketReader::new();
        let de_packet = reader.deserialize(&mut bytes).unwrap();
        assert_eq!(packet, de_packet.payload);
    }
}
