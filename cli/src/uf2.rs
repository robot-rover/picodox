use anyhow::{bail, Result};
use bitflags::bitflags;
use zerocopy::{try_transmute, FromBytes, Immutable, KnownLayout};

const UF2_MAGIC_START0: [u8; 4] = [0x55, 0x46, 0x32, 0x0A];
const UF2_MAGIC_START1: [u8; 4] = [0x57, 0x51, 0x5D, 0x9E];
const UF2_MAGIC_END: [u8; 4] = [0x30, 0x6F, 0xB1, 0x0A];

#[derive(Debug, FromBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct Uf2Block {
    magic0: [u8; 4],
    magic1: [u8; 4],
    flags_data: u32,
    target_addr: u32,
    payload_size: u32,
    block_num: u32,
    num_blocks: u32,
    extra_data: u32,
    payload: [u8; 476],
    magic2: [u8; 4],
}

impl Uf2Block {
    pub fn get_flags(&self) -> Uf2Flags {
        Uf2Flags::from_bits(self.flags_data)
            .expect("This should be checked when Uf2Block is constructed")
    }

    pub fn get_payload(&self) -> &[u8] {
        &self.payload[..self.payload_size as usize]
    }

    pub fn get_block_num(&self) -> u32 {
        self.block_num
    }

    pub fn get_extra_data(&self) -> u32 {
        self.extra_data
    }

    pub fn get_num_blocks(&self) -> u32 {
        self.num_blocks
    }

    pub fn get_bounds(&self) -> (u32, u32) {
        (self.target_addr, self.target_addr + self.payload_size)
    }

    pub fn parse(data: &[u8]) -> anyhow::Result<Vec<Self>> {
        if data.len() % 512 != 0 {
            bail!("Invalid UF2 block size ({} % 512 == {})", data.len(), data.len() % 512);
        }

        data.chunks_exact(512).map(|chunk| {
            let mut target: [u8; 512] = [0; 512];
            target.clone_from_slice(chunk);

            let block_res: Result<Uf2Block, _> = try_transmute!(target);
            let block = match block_res {
                Ok(block) => block,
                Err(err) => bail!("Unable to parse bytes to uf2 header: {:?}", err),
            };

            if block.magic0 != UF2_MAGIC_START0 {
                bail!("Invalid UF2 header (magic0: {:x?})", block.magic0);
            }
            if block.magic1 != UF2_MAGIC_START1 {
                bail!("Invalid UF2 header (magic1: {:x?})", block.magic1);
            }
            if block.magic2 != UF2_MAGIC_END {
                bail!("Invalid UF2 header (magic2: {:x?})", block.magic2);
            }

            if Uf2Flags::from_bits(block.flags_data).is_none() {
                bail!("Invalid UF2 flags ({:x?})", block.flags_data);
            }

            if block.payload_size > 476 {
                bail!("Invalid UF2 payload size ({})", block.payload_size);
            }

            Ok(block)
        }).collect::<Result<Vec<_>, _>>()
    }
}

bitflags! {
    pub struct Uf2Flags: u32 {
        const NotMainFlash  = 0x00000001;
        const FileContainer = 0x00001000;
        const FamilyIdPres  = 0x00002000;
        const ChecksumPres  = 0x00004000;
        const ExtTagsPres   = 0x00008000;
    }
}

#[derive(FromBytes)]
#[repr(C)]
struct Uf2Checksum {
    start_addr: u32,
    num_blocks: u32,
    checksum: [u8; 16],
}

#[derive(FromBytes)]
#[repr(C)]
struct Uf2Tag {
    len: u8,
    designator: [u8; 3],
    payload: [u8],
}
