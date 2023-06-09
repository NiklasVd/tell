use core::fmt;
use std::{error::Error, io, net::SocketAddr, any::Any};
use crossbeam_channel::{TrySendError, TryRecvError};
use rmp_serde::{decode, encode};

use crate::{event::UdpAdapterEvent, net::adapter::SendCommand};

pub type TResult<T = ()> = Result<T, TellErr>;

pub enum TellErr {
    Lib(LibErr),
    Io(io::Error),
    Encode(encode::Error),
    Decode(decode::Error),
    ChannelSend(Box<dyn Any + 'static + Send + Sync>),
    ChannelRecv(TryRecvError),
    Other(Box<dyn Any + 'static + Send>)
}

impl Error for TellErr {
}

impl fmt::Debug for TellErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TellErr::Lib(e) => write!(f, "{e}"),
            TellErr::Io(e) => write!(f, "{e}"),
            TellErr::Encode(e) => write!(f, "{e}"),
            TellErr::Decode(e) => write!(f, "{e}"),
            TellErr::ChannelSend(e) => write!(f, "{:?}", e),
            TellErr::ChannelRecv(e) => write!(f, "{e}"),
            TellErr::Other(e) => write!(f, "{:?}", e)
        }
    }
}

impl fmt::Display for TellErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ---- From conversions ----

impl From<std::io::Error> for TellErr {
    fn from(value: std::io::Error) -> Self {
        TellErr::Io(value)
    }
}

impl From<encode::Error> for TellErr {
    fn from(value: encode::Error) -> Self {
        TellErr::Encode(value)
    }
}

impl From<decode::Error> for TellErr {
    fn from(value: decode::Error) -> Self {
        TellErr::Decode(value)
    }
}

impl From<TrySendError<UdpAdapterEvent>> for TellErr {
    fn from(value: TrySendError<UdpAdapterEvent>) -> Self {
        TellErr::ChannelSend(Box::new(value))
    }
}

impl From<TrySendError<SendCommand>> for TellErr {
    fn from(value: TrySendError<SendCommand>) -> Self {
        TellErr::ChannelSend(Box::new(value))
    }
}

impl From<TryRecvError> for TellErr {
    fn from(value: TryRecvError) -> Self {
        TellErr::ChannelRecv(value)
    }
}

impl From<Box<dyn Any + 'static + Send>> for TellErr {
    fn from(value: Box<dyn Any + 'static + Send>) -> Self {
        TellErr::Other(value)
    }
}

// ----

#[derive(Debug)]
pub enum LibErr {
    InvalidTimestamp(u128, u128),
    InvalidName(String),
    InvalidPacketType(String),
    PeerAlreadyConnected(SocketAddr),
    PeerNotConnected(SocketAddr),
    MaxConnectionsReached(usize),
    NotConnected
}

impl fmt::Display for LibErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
