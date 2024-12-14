use heapless::Vec;
use crate::{errors::ProtoError, WireSize};
use serde::{de::DeserializeOwned, Serialize};
use postcard;
use crc::{Crc, CRC_8_BLUETOOTH};
use cobs;

const CRC: Crc<u8> = Crc::<u8>::new(&CRC_8_BLUETOOTH);

pub fn cs_encode<S: Serialize + WireSize, const N: usize>(value: &S) -> Result<Vec<u8, N>, ProtoError> {
    if N < S::CS_MAX_SIZE {
        return Err(ProtoError::buffer_size());
    }

    let mut buf = postcard::to_vec::<S, N>(&value)?;

    let crc = CRC.checksum(&buf);
    buf.push(crc).map_err(|_| ProtoError::invariant(0x1))?;

    Ok(buf)
}

pub fn wire_encode<S: Serialize + WireSize, const N: usize>(value: &S) -> Result<Vec<u8, N>, ProtoError> {
    if N < S::WIRE_MAX_SIZE {
        return Err(ProtoError::buffer_size());
    }

    let buf = cs_encode::<S, N>(value)?;

    let mut cobs_buf: Vec<u8, N> = Vec::new();
    cobs_buf.resize(N, 0).map_err(|_| ProtoError::invariant(0x2))?;
    let result_len = cobs::try_encode(&buf, &mut cobs_buf).map_err(|_| ProtoError::invariant(0x3))?;
    cobs_buf.truncate(result_len);
    cobs_buf.push(0).map_err(|_| ProtoError::invariant(0x4))?;

    Ok(cobs_buf)
}

pub fn cs_decode<D: DeserializeOwned + WireSize>(buf: &mut [u8]) -> Result<D, ProtoError> {
    let new_len = buf.len();

    if new_len == 0 {
        return Err(ProtoError::bad_length(new_len));
    }

    let actual_crc = buf[new_len - 1];
    let message_buf = &mut buf[..new_len - 1];

    // Calculate and extract CRCs
    let calc_crc = CRC.checksum(&message_buf);

    if actual_crc != calc_crc {
        Err(ProtoError::CrcMismatch {
            calculated: calc_crc,
            actual: actual_crc,
        })
    } else {
        // Finally, decode the message
        Ok(postcard::from_bytes(message_buf)?)
    }
}

pub fn wire_decode<D: DeserializeOwned + WireSize>(buf: &mut [u8]) -> Result<D, ProtoError> {
    // COBS decode
    if buf.last() != Some(&0u8) {
        return Err(ProtoError::invariant(0x5));
    }
    let without_sentinel = buf.len() - 1;
    let no_sentinel_buf = &mut buf[..without_sentinel];

    let new_len = cobs::decode_in_place(no_sentinel_buf).map_err(|_| ProtoError::invariant(0x6))?;

    cs_decode::<D>(&mut no_sentinel_buf[..new_len])
}
