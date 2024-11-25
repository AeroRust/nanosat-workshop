//! SD Card storage implementation
//!
//! This is a raw write to the SD card.
//! It should be fully formatted and without a filesystem!
//!
//! We use the following schema for each block of 512 bytes:
//!
//! 1. 2 bytes (big-endian) - u16, CRC of the whole content of the block
//!   (incl. msg count, see below, and excluding the CRC and empty bytes)
//! 2. 2 bytes (big-endian) - u16, total recorded messages in block:
//! 3. 2 bytes (big-endian) - u16, total written messages length in the block.
//!    Indicates how much bytes of the block have been occupied with actual data.
//! 4. Messages - individually encoded messages using [`postcard`]
//! 5. Empty bytes are filled with value `255`

#[cfg(feature = "defmt-03")]
use defmt::trace;

use embedded_hal_async::spi::SpiDevice;
use embedded_sdmmc::{
    blockdevice::AsyncBlockDevice, sdcard::Async, Block, BlockCount, BlockIdx, SdCard, SdCardError,
};

use num::ToPrimitive;
use serde::{Deserialize, Serialize};

pub const EMPTY_VALUE: u8 = 255;
pub const EMPTY_BLOCK: embedded_sdmmc::Block = Block {
    contents: [EMPTY_VALUE; Block::LEN],
};

pub const CRC_HEADER: usize = 2;
pub const MESSAGES_COUNT_HEADER: usize = 2;
/// the amount of Messages bytes written in the block.
pub const MESSAGES_LEN_HEADER: usize = 2;
pub const CONTENT_SIZE: usize =
    Block::LEN - CRC_HEADER - MESSAGES_COUNT_HEADER - MESSAGES_LEN_HEADER;

pub const X25: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_IBM_SDLC);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record<T> {
    /// ms since start up time
    pub timestamp: u64,
    pub message: T,
}

impl<'a, T> Record<T>
where
    T: Serialize + Deserialize<'a>,
{
    pub fn to_storage<'b>(&self, buf: &'b mut [u8]) -> Result<&'b mut [u8], postcard::Error> {
        postcard::to_slice(self, buf)
    }

    pub fn from_storage(bytes: &'a [u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub enum Error {
    Postcard(postcard::Error),
    SdCard(SdCardError),
    EoFNotFound(File),
}

impl From<SdCardError> for Error {
    fn from(value: SdCardError) -> Self {
        Error::SdCard(value)
    }
}

impl From<postcard::Error> for Error {
    fn from(value: postcard::Error) -> Self {
        Error::Postcard(value)
    }
}

pub struct Filesystem<SPI, DELAYER> {
    sd_card: SdCard<SPI, DELAYER, Async>,
    // files: FnvIndexSet<>,
}

impl<SPI, DELAYER> Filesystem<SPI, DELAYER> {
    pub fn new(sd_card: SdCard<SPI, DELAYER, Async>) -> Self
    where
        SPI: SpiDevice,
        DELAYER: embedded_hal_async::delay::DelayNs,
    {
        Self { sd_card }
    }
}

impl<SPI, DELAYER> Filesystem<SPI, DELAYER>
where
    SPI: SpiDevice,
    DELAYER: embedded_hal_async::delay::DelayNs,
{
    pub async fn persist<'a, const N: usize, T>(
        &self,
        buf: &mut [Block; N],
        buf_fill: &mut [usize; N],
        record: Record<T>,
        write_block_idx: &mut BlockIdx,
    ) -> Result<usize, Error>
    where
        T: serde::Serialize + serde::Deserialize<'a>,
    {
        persist(&self.sd_card, buf, buf_fill, record, write_block_idx).await
    }

    pub async fn find_eof(&self, file: File, blocks: &mut [Block]) -> Option<BlockIdx> {
        let total_blocks = file.total_blocks();

        // how many elements to search through on every lookup
        // before jumping to next looking place (forwards or backwards)
        let forward_n = BlockCount(200);

        let denom_table = Self::denom_table(forward_n.0 as usize, total_blocks as usize);

        let mut looking_index = BlockIdx(file.start_idx);
        let mut last_empty_index = None;
        let mut is_first_lookup = true;

        for table_denom in denom_table {
            let table_denom = BlockCount(table_denom);
            // make sure we don't get out of bound if we are at the very end of the searched indices (start_i & end_i)
            let end_index = BlockIdx(file.end_idx).min(looking_index + forward_n);

            let find_empty = self
                .find_failing_crc_block(looking_index, end_index, blocks)
                .await;

            match (find_empty, is_first_lookup) {
                // need to look forwards
                (None, _) => {
                    // if we are at the start, and we don't find an empty block
                    // we go to the middle (start + 1/2)

                    // if we are at 1/4 to end and we don't find empty block
                    // we skip 1/8 blocks forward

                    let next_looking_index = looking_index + table_denom.max(forward_n);
                    #[cfg(feature = "defmt-03")]
                    trace!("(forwards) Next looking index: {}", next_looking_index);
                    looking_index += table_denom.max(forward_n);
                }
                // need to look backwards
                // if we are at the first lookup and we find the empty block,
                // then it's in the first `forward_n` blocks from the start
                // we found our end-of-file!
                (Some(index), true) => {
                    #[cfg(feature = "defmt-03")]
                    trace!("EoF found at: {}", index);

                    return Some(index);
                }
                (Some(index), false) if index == looking_index => {
                    // set the current looking index as the last empty index
                    // just in case this is the block that is the first empty block
                    last_empty_index = Some(looking_index);

                    #[cfg(feature = "defmt-03")]
                    trace!("last_empty is set to: {:?}", last_empty_index);

                    let next_looking_index = looking_index - table_denom.max(forward_n);
                    #[cfg(feature = "defmt-03")]
                    trace!(
                        "(backwards) looking_index = {}; next_looking_index = {}",
                        looking_index,
                        next_looking_index
                    );

                    // update the looking index backwards
                    looking_index -= table_denom.max(forward_n);
                }
                // we found our end-of-file!
                (Some(index), false) => {
                    #[cfg(feature = "defmt-03")]
                    trace!("EoF found at: {}", index);

                    return Some(index);
                }
            }
            is_first_lookup = false;
        }

        // if we end-up not finding any other empty index, this means that the last_empty_index
        // we found is indeed the End-of-file
        // this can happen if our EoF is at the first lookup index of the `forward_n` lookup elements
        last_empty_index
    }

    pub async fn find_failing_crc_block(
        &self,
        start_block: BlockIdx,
        end_block: BlockIdx,
        blocks_buf: &mut [Block],
    ) -> Option<BlockIdx> {
        // how many empty (CRC check value is missing) consecutive blocks should we have,
        // before we return the BlockIdx of the first
        let consecutive_check = 3;
        let mut first_block = None;
        let mut consecutive = 0_u8;

        let blocks_buf_len = blocks_buf.len() as u32;
        let blocks_chunks =
            num::rational::Ratio::new(end_block.0 - start_block.0, blocks_buf_len).ceil();

        for chunk_index in 0..blocks_chunks.to_u32().unwrap() {
            let block_idx = BlockIdx(start_block.0 + chunk_index * blocks_buf_len);

            'block_read: loop {
                let result = self
                    .sd_card
                    .read(blocks_buf, block_idx, "Looking for empty block with no CRC")
                    .await;

                if let Err(_err) = result {
                    // try to read the same block again
                    continue 'block_read;
                } else {
                    break 'block_read;
                }
            }

            for block in blocks_buf.iter() {
                let block_crc = u16::from_be_bytes([block.contents[0], block.contents[1]]);
                let msgs_len = u16::from_be_bytes([block.contents[2], block.contents[3]]) as usize;

                // Check if CRC is valid
                // if so, then this is not our End of File block!
                let crc_content = &block.contents[CRC_HEADER..][..msgs_len];
                let expected_crc = X25.checksum(crc_content);
                // make sure to check if the block is empty
                let crc_valid = (block_crc == expected_crc) && expected_crc != 0;
                // only if the crc value doesn't match,
                // we might have found our end-of-file
                #[cfg(feature = "defmt-03")]
                trace!(
                "Is CRC valid for block {:?}; CRC: {:x}; expected: {:x}; algorithm check: {:x} - CRC Ok? {}",
                block_idx,
                block_crc,
                expected_crc,
                X25.algorithm.check,
                crc_valid,
            );

                if !crc_valid {
                    // mark the first block in X consecutive
                    // that doesn't have the CRC check value
                    if first_block.is_none() {
                        first_block = Some(block_idx);
                    }
                    // increase the consecutive count
                    consecutive += 1;

                    if consecutive == consecutive_check {
                        #[cfg(feature = "defmt-03")]
                        trace!("Empty block found - {}", first_block);
                        // return the first empty block which we've marked.
                        return first_block;
                    }
                } else {
                    // reset the consecutive counter
                    consecutive = 0;
                }
            }
        }

        None
    }

    /// For 128 GiB (`1073741824` bytes) SD card we have `2_097_152` blocks.
    ///
    /// Even when reading 1 block we still have a maximum of 21 records in the denom table.
    pub fn denom_table(read_blocks: usize, total_blocks_to_read: usize) -> heapless::Vec<u32, 21> {
        // will be rounded down
        let find_log_denom = read_blocks.ilog2();

        let log_blocks = total_blocks_to_read.ilog2();

        let denom_powers = log_blocks - find_log_denom;

        (1..=denom_powers)
            .map(|power| 2_u32.pow(power))
            // collecting into the heapless vec is safe for up to 128 GiB!
            .collect::<heapless::Vec<_, 21>>()
    }
}

/// `write_block_idx` - the id of the block for the next write
///
/// # Returns
///
/// The amount of blocks (`N`) written to the SD card when the buffer cannot take
/// the current message and flushes the buffer to the SD card.
///
/// If the buffer hasn't been sent to the SD card, then `0` will be returned.
pub async fn persist<'a, const N: usize, SPI, DELAYER, T>(
    sd_card: &SdCard<SPI, DELAYER, Async>,
    buf: &mut [Block; N],
    buf_fill: &mut [usize; N],
    record: Record<T>,
    write_block_idx: &mut BlockIdx,
) -> Result<usize, Error>
where
    SPI: SpiDevice,
    DELAYER: embedded_hal_async::delay::DelayNs,
    T: serde::Serialize + serde::Deserialize<'a>,
{
    let ser_size = postcard::experimental::serialized_size(&record)?;

    let block_index_to_fill = buf_fill
        .iter()
        .enumerate()
        .find(|(_block_index, written_to)| CONTENT_SIZE - *written_to > ser_size)
        .map(|(block_index, _)| block_index);

    match block_index_to_fill {
        // there's space in the 4 KiB buffer
        Some(block_index) => {
            let current_block_content_index =
                CRC_HEADER + MESSAGES_COUNT_HEADER + MESSAGES_LEN_HEADER + buf_fill[block_index];
            buf_fill[block_index] += ser_size;
            // increment the messages count in the block
            // first get the current count
            let current_msg_count = u16::from_be_bytes({
                let mut bytes = [0_u8; 2];
                bytes.copy_from_slice(
                    &buf[block_index].contents[CRC_HEADER..][..MESSAGES_COUNT_HEADER],
                );
                bytes
            });
            // increment and copy the new value to the header
            buf[block_index][CRC_HEADER..][..MESSAGES_COUNT_HEADER]
                .copy_from_slice(&(current_msg_count + 1).to_be_bytes());
            // add the total length of the written messages bytes in the header
            buf[block_index][CRC_HEADER + MESSAGES_COUNT_HEADER..][..MESSAGES_LEN_HEADER]
                .copy_from_slice(&(buf_fill[block_index] as u16).to_be_bytes());

            // exclude the headers, add the serialized content to the right spot in the buffer
            // and leave any empty bytes at the end untouched
            let slice_to_write =
                &mut buf[block_index].contents[current_block_content_index..][..ser_size];
            record.to_storage(slice_to_write)?;

            // no bytes were actually written to the sd card
            Ok(0)
        }
        // no space in the buffer
        None => {
            // 0. Calculate u16 CRC for each block
            for (block_index, block) in buf.iter_mut().enumerate() {
                let crc_content_len = &block[CRC_HEADER..buf_fill[block_index]];

                let block_crc = X25.checksum(crc_content_len);
                block[..CRC_HEADER].copy_from_slice(&block_crc.to_be_bytes());
            }
            // 1. flush, aka send blocks buffer to SD card for writing
            sd_card.write(buf, *write_block_idx).await?;
            write_block_idx.0 = (*write_block_idx + embedded_sdmmc::BlockCount(N as u32)).0;

            // 1.1 clear data from buffers
            buf.fill(EMPTY_BLOCK);
            // clear fill buffer back to 0 bytes written for each Block
            buf_fill.fill(0);
            // 2. add data to the cleared buffer
            // copy-paste of the Some branch:
            let block_index = 0;
            let current_block_content_index =
                CRC_HEADER + MESSAGES_COUNT_HEADER + MESSAGES_LEN_HEADER + buf_fill[block_index];
            buf_fill[block_index] += ser_size;
            // increment the messages count in the block
            // first get the current count
            let current_msg_count = u16::from_be_bytes({
                let mut bytes = [0_u8; 2];
                bytes.copy_from_slice(
                    &buf[block_index].contents[CRC_HEADER..][..MESSAGES_COUNT_HEADER],
                );
                bytes
            });
            // increment and copy the new value to the header
            buf[block_index][CRC_HEADER..][..MESSAGES_COUNT_HEADER]
                .copy_from_slice(&(current_msg_count + 1).to_be_bytes());
            // add the total length of the written messages bytes in the header
            buf[block_index][CRC_HEADER + MESSAGES_COUNT_HEADER..][..MESSAGES_LEN_HEADER]
                .copy_from_slice(&(buf_fill[block_index] as u16).to_be_bytes());

            // exclude the headers, add the serialized content to the right spot in the buffer
            // and leave any empty bytes at the end untouched
            let slice_to_write =
                &mut buf[block_index].contents[current_block_content_index..][..ser_size];
            record.to_storage(slice_to_write)?;

            Ok(N)
        }
    }
}

/// A files consists of starting and ending block idx.
///
/// NB: Keep in mind that we do not guard against invalid ranges,
/// e.g. if start_idx is after end_idx.
/// This implementation is kept to a minimum for ease of development
/// and inner workings.
///
/// # Examples
///
/// ```
/// use postcard::experimental::max_size::MaxSize;
/// use protocol::storage::File;
///
/// // Invalid and unrealistic example showing the max serialization size.
/// let maxed_out_size = postcard::experimental::serialized_size(&File {
///     start_idx: u32::MAX,
///     end_idx: u32::MAX,
/// })
/// .expect("Should get serialized size for maxed out File");
///
/// assert_eq!(10, maxed_out_size);
/// let somewhat_minimum_size = postcard::experimental::serialized_size(&File {
///     // we use 1 as in the real impl 0 should be the files index/filed state
///     start_idx: 1,
///     end_idx: u32::MAX,
/// })
/// .expect("Should get serialized size for minimum File");
///
/// assert_eq!(6, somewhat_minimum_size);
///
/// assert_eq!(10, File::POSTCARD_MAX_SIZE);
/// ```
// BlockIdx does not support `serde`, hence doesn't implement `Serialize` & `Deserialize`
#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    postcard::experimental::max_size::MaxSize,
    core::hash::Hash,
)]
#[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
pub struct File {
    pub start_idx: u32,
    pub end_idx: u32,
}

impl File {
    /// Create a new file between 2 blocks.
    ///
    /// It's good practice to keep the low block as a start block
    /// but [`File::new`] will make sure to make it for you
    pub fn new(between_a: u32, between_b: u32) -> Self {
        Self {
            start_idx: between_a.min(between_b),
            end_idx: between_a.max(between_b),
        }
    }

    // slower as performs division and unsafe casting!
    pub fn new_addresses(address_a: u64, address_b: u64) -> Self {
        Self {
            start_idx: (address_a.min(address_b) / 512) as u32,
            end_idx: (address_a.max(address_b) / 512) as u32,
        }
    }

    pub fn total_blocks(&self) -> u32 {
        self.end_idx - self.start_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use postcard::experimental::max_size::MaxSize;

    #[test]
    fn test_file_struct_serialization_size() {
        let maxed_out_size = postcard::experimental::serialized_size(&File {
            start_idx: u32::MAX,
            end_idx: u32::MAX,
        })
        .expect("Should get serialized size for maxed out File");

        assert_eq!(10, maxed_out_size);
        let somewhat_minimum_size = postcard::experimental::serialized_size(&File {
            start_idx: 1,
            end_idx: u32::MAX,
        })
        .expect("Should get serialized size for minimum File");

        assert_eq!(6, somewhat_minimum_size);

        assert_eq!(10, File::POSTCARD_MAX_SIZE);
    }
}
