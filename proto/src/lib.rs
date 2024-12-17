#![no_std]

use errors::ProtoError;
use heapless::Vec;
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

pub mod errors;
pub mod proto_impl;

pub trait WireSize {
    const WIRE_MAX_SIZE: usize;
    const CS_MAX_SIZE: usize;
}

const fn cobs_max_length(source_len: usize) -> usize {
    source_len + (source_len / 254) + if source_len % 254 > 0 { 1 } else { 0 }
}

impl<T: MaxSize> WireSize for T {
    // Wire is postcard with a CRC that is then COBS encoded and has a \0 sentinel
    // Pre COBS length is the max postcard length plus the CRC byte
    // Then we add one more byte for the sentinel
    const WIRE_MAX_SIZE: usize = cobs_max_length(T::POSTCARD_MAX_SIZE + 1) + 1;
    // If there is no cobs encoding (not necessary in a framed format such as I2C), then
    // the only overhead on top of postcard is the CRC byte
    const CS_MAX_SIZE: usize = T::POSTCARD_MAX_SIZE + 1;
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub struct Version {
    major: u16,
    minor: u16,
}

pub const CURRENT_VERSION: Version = Version { major: 0, minor: 0 };

pub const DATA_COUNT: usize = 8;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum Command {
    Reset,
    UsbDfu,
    FlashFw { count: u32 },
    EchoMsg { count: u16 },
    Data([u8; DATA_COUNT]),
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum AckType {
    AckReset,
    AckUsbDfu,
    AckFlashFw,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum NackType {
    Unexpected,
    PacketErr(ProtoError),
    BufferOverflow,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum Response {
    Ack(AckType),
    Nack(NackType),
    EchoMsg { count: u16 },
    Data([u8; DATA_COUNT]),
}

pub const MAX_KEYS: usize = 35;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum KeyResponse {
    Response(Response),
    KeyUpdate(Vec<u8, MAX_KEYS>),
}

impl KeyResponse {
    pub fn keys<const N: usize>(key_codes: [u8; N]) -> Self {
        let mut vec = Vec::new();
        vec.extend_from_slice(&key_codes)
            .expect("key_codes is too long");
        KeyResponse::KeyUpdate(vec)
    }

    pub const fn no_keys() -> Self {
        KeyResponse::KeyUpdate(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use postcard::to_stdvec;

    #[test]
    fn check_enum_size() {
        let short = Response::EchoMsg { count: 12 };
        let short_bytes = to_stdvec(&short).expect("Cannot serialize short response");
        let long = Response::Data([1u8; DATA_COUNT]);
        let long_bytes = to_stdvec(&long).expect("Cannot serialize long response");
        assert_eq!(short_bytes.len(), 2);
        assert_eq!(long_bytes.len(), 9);
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
    struct TestArrStruct([u8; 10]);

    #[test]
    fn check_vec_size() {
        let mut short_vec = Vec::new();
        short_vec.push(1u8).unwrap();
        let short_bytes =
            to_stdvec(&KeyResponse::KeyUpdate(short_vec)).expect("Cannot serialize short vec");

        let long_arr = TestArrStruct([0u8; 10]);
        let long_bytes = to_stdvec(&long_arr).expect("Cannot serialize long vec");

        // One byte for KeyResponse discriminant, one for vec len, and one for the data byte
        assert_eq!(short_bytes.len(), 3);
        assert_eq!(long_bytes.len(), 10);
    }
}
