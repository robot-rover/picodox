#![no_std]

use errors::ProtoError;
use postcard::experimental::max_size::MaxSize;
use serde::{Serialize, Deserialize};

pub mod errors;
pub mod proto_impl;

pub trait WireSize {
    const WIRE_MAX_SIZE: usize;
}

const fn cobs_max_length(source_len: usize) -> usize {
    source_len + (source_len / 254) + if source_len % 254 > 0 { 1 } else { 0 }
}

impl<T: MaxSize> WireSize for T {
    // Wire is postcard with a CRC that is then COBS encoded and has a \0 sentinel
    // Pre COBS length is the max postcard length plus the CRC byte
    // Then we add one more byte for the sentinel
    const WIRE_MAX_SIZE: usize = cobs_max_length(T::POSTCARD_MAX_SIZE + 1) + 1;
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub struct Version {
    major: u16,
    minor: u16,
}

pub const CURRENT_VERSION: Version = Version {
    major: 0,
    minor: 0,
};

pub const DATA_COUNT: usize = 8;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum Command {
    Reset,
    UsbDfu,
    EchoMsg {
        count: u16,
    },
    Data([u8; DATA_COUNT]),
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum AckType {
    AckReset,
    AckUsbDfu,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum Response {
    Ack(AckType),
    LogMsg {
        count: u16,
    },
    EchoMsg {
        count: u16,
    },
    Data([u8; DATA_COUNT]),
    PacketErr(ProtoError),
}


#[cfg(test)]
mod tests {
    use super::*;

    use postcard::to_stdvec;

    #[test]
    fn check_enum_size() {
        let short = Response::LogMsg { count: 12 };
        let short_bytes = to_stdvec(&short)
            .expect("Cannot serialize short response");
        let long = Response::Data([1u8; DATA_COUNT]);
        let long_bytes = to_stdvec(&long)
            .expect("Cannot serialize long response");
        assert_eq!(short_bytes.len(), 2);
        assert_eq!(long_bytes.len(), 9);
    }
}
