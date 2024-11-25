use serde::{Serialize, Deserialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    major: u16,
    minor: u16,
}

pub const CURRENT_VERSION: Version = Version {
    major: 0,
    minor: 0,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Reset,
    FlashFw,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    LogMsg {
        bytes_count: u16,
    },
    Data([u8; 8]),
    PacketErr,
}


#[cfg(test)]
mod tests {
    use super::*;

    use postcard::to_stdvec;

    #[test]
    fn check_enum_size() {
        let short = Response::LogMsg { bytes_count: 12 };
        let short_bytes = to_stdvec(&short)
            .expect("Cannot serialize short response");
        let long = Response::Data([1u8; 8]);
        let long_bytes = to_stdvec(&long)
            .expect("Cannot serialize long response");
        assert_eq!(short_bytes.len(), 2);
        assert_eq!(long_bytes.len(), 9);
    }
}
