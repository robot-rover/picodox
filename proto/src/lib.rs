use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Version {
    major: u16,
    minor: u16,
}

pub const CURRENT_VERSION: Version = Version {
    major: 0,
    minor: 0,
};

pub enum Command {
    Reset,
    FlashFw,
}

pub enum Response {
    LogMsg {
        bytes_count: u16,
    },
    Data([u8; 8]),
}


#[cfg(test)]
mod tests {
    use super::*;

    use postcard::to_vec;

    #[test]
    fn check_enum_size() {
        let short = Response::LogMsg { bytes_count: 12 };
        let short_bytes = to_vec(&short);
        let long = Response::Data([1u8; 8]);
        let long_bytes = to_vec(&long);
    }
}
