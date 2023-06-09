use core::fmt;
use std::time::{SystemTime, UNIX_EPOCH, Instant};

pub fn timestamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    pub bytes_transfer: u128,
    pub packets_transfer: u64,
    pub last_transfer: Instant
}

impl Metrics {
    pub fn new() -> Metrics {
        Metrics {
            bytes_transfer: 0, packets_transfer: 0, last_transfer: Instant::now()
        }
    }

    pub fn transfer(&mut self, size: usize) {
        self.bytes_transfer += size as u128;
        self.packets_transfer += 1;
        self.last_transfer = Instant::now();
    }
}

impl fmt::Debug for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} packets ({}b) since {:.2}s", self.packets_transfer, self.bytes_transfer,
            self.last_transfer.elapsed().as_secs_f32())
    }
}
