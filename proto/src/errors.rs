use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub struct Ucid(pub u8);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum ProtoError {
    BufferSize {
        ucid: Ucid,
    },
    PostcardError(u8),
    CrcMismatch {
        calculated: u8,
        actual: u8,
    },
    BadLength {
        ucid: Ucid,
        len: u8,
    },
    Invariant {
        ucid: Ucid,
        kind: u8,
    },
}

impl ProtoError {
    pub fn buffer_size(ucid: Ucid) -> Self {
        ProtoError::BufferSize { ucid }
    }

    pub fn crc_mismatch(calculated: u8, actual: u8) -> Self {
        ProtoError::CrcMismatch { calculated, actual }
    }

    pub fn bad_length(ucid: Ucid, len: usize) -> Self {
        ProtoError::BadLength {
            ucid,
            len: len.try_into().unwrap_or(u8::MAX),
        }

    }

    pub fn invariant(ucid: Ucid, kind: u8) -> Self {
        ProtoError::Invariant { ucid, kind }
    }
}

impl From<postcard::Error> for ProtoError {
    fn from(err: postcard::Error) -> Self {
        ProtoError::PostcardError(err as u8)
    }
}

