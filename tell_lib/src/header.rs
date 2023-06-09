use serde::{Serialize, Deserialize};

use crate::{id::Id, util::timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PacketHeader {
    source: Id,
    timestamp: u128
}

impl PacketHeader {
    pub fn new(source: Id) -> PacketHeader {
        PacketHeader {
            source, timestamp: timestamp()
        }
    }

    pub fn source(&self) -> &Id {
        &self.source
    }
}
