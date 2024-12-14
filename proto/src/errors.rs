use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, MaxSize)]
pub enum ProtoError {
    BufferSize,
    PostcardError(u8),
    CrcMismatch { calculated: u8, actual: u8 },
    BadLength { len: u8 },
    Invariant { kind: u8 },
}

impl ProtoError {
    pub fn buffer_size() -> Self {
        ProtoError::BufferSize
    }

    pub fn crc_mismatch(calculated: u8, actual: u8) -> Self {
        ProtoError::CrcMismatch { calculated, actual }
    }

    pub fn bad_length(len: usize) -> Self {
        ProtoError::BadLength {
            len: len.try_into().unwrap_or(u8::MAX),
        }
    }

    pub fn invariant(kind: u8) -> Self {
        ProtoError::Invariant { kind }
    }
}

impl From<postcard::Error> for ProtoError {
    fn from(err: postcard::Error) -> Self {
        ProtoError::PostcardError(err as u8)
    }
}
