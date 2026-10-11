//! A minimal Ogg page writer, used to place Opus packets in an Ogg stream.
//!
//! One packet is written per page. A page is a 27-byte header, a segment table of lacing values,
//! and the packet data (RFC 3533). The 32-bit CRC field covers the whole page, with the field
//! itself zeroed while the CRC is computed.

use std::io::Write;

use super::EncodeFailure;

/// The Ogg CRC-32 polynomial, applied with the most significant bit first.
const CRC_POLYNOMIAL: u32 = 0x04c1_1db7;

/// The page flag for the first page of a logical stream.
pub(super) const FLAG_BEGIN_OF_STREAM: u8 = 0x02;

/// The page flag for the last page of a logical stream.
pub(super) const FLAG_END_OF_STREAM: u8 = 0x04;

/// The most segments a page can hold, because the segment table is one byte per segment.
const MAX_SEGMENTS: usize = 255;

/// Writes the pages of one logical Ogg stream.
pub(super) struct PageWriter<W: Write> {
    output: W,
    serial: u32,
    sequence: u32,
}

impl<W: Write> PageWriter<W> {
    /// Wrap `output` as one logical stream with the given serial number.
    pub(super) fn new(output: W, serial: u32) -> Self {
        Self {
            output,
            serial,
            sequence: 0,
        }
    }

    /// Write one page that holds one complete packet.
    ///
    /// `granule` is the granule position of the last sample the packet completes, and `flags` is
    /// the page header flag byte.
    pub(super) fn write_packet(
        &mut self,
        packet: &[u8],
        granule: u64,
        flags: u8,
    ) -> Result<(), EncodeFailure> {
        let segments = lacing(packet.len());
        if segments.len() > MAX_SEGMENTS {
            return Err(EncodeFailure::Message(format!(
                "an Ogg page cannot hold a packet of {} bytes",
                packet.len()
            )));
        }

        let mut page = Vec::with_capacity(27 + segments.len() + packet.len());
        page.extend_from_slice(b"OggS");
        page.push(0); // stream structure version
        page.push(flags);
        page.extend_from_slice(&granule.to_le_bytes());
        page.extend_from_slice(&self.serial.to_le_bytes());
        page.extend_from_slice(&self.sequence.to_le_bytes());
        page.extend_from_slice(&[0, 0, 0, 0]); // CRC placeholder
        page.push(segments.len() as u8);
        page.extend_from_slice(&segments);
        page.extend_from_slice(packet);

        let crc = crc32(&page);
        page[22..26].copy_from_slice(&crc.to_le_bytes());
        self.output.write_all(&page)?;
        self.sequence = self.sequence.wrapping_add(1);
        Ok(())
    }
}

/// The lacing values for a packet of `len` bytes: one 255 for each full 255-byte segment, then the
/// remainder. A remainder of zero still adds a zero lacing value, to terminate the packet.
fn lacing(len: usize) -> Vec<u8> {
    let mut segments = Vec::with_capacity(len / 255 + 1);
    let mut remaining = len;
    while remaining >= 255 {
        segments.push(255);
        remaining -= 255;
    }
    segments.push(remaining as u8);
    segments
}

/// The Ogg CRC-32 of a page.
fn crc32(page: &[u8]) -> u32 {
    let mut crc = 0u32;
    for byte in page {
        crc ^= u32::from(*byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ CRC_POLYNOMIAL
            } else {
                crc << 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lacing_splits_a_packet_at_each_full_255_byte_segment() {
        assert_eq!(lacing(0), vec![0]);
        assert_eq!(lacing(1), vec![1]);
        assert_eq!(lacing(254), vec![254]);
        assert_eq!(lacing(255), vec![255, 0]);
        assert_eq!(lacing(510), vec![255, 255, 0]);
        assert_eq!(lacing(511), vec![255, 255, 1]);
    }

    #[test]
    fn a_page_has_the_capture_pattern_and_a_crc_over_the_whole_page() {
        let mut bytes = Vec::new();
        let mut writer = PageWriter::new(&mut bytes, 0x1234_5678);
        writer
            .write_packet(b"hello", 960, FLAG_BEGIN_OF_STREAM)
            .expect("write the page");

        assert_eq!(&bytes[0..4], b"OggS");
        assert_eq!(bytes[4], 0, "version");
        assert_eq!(bytes[5], FLAG_BEGIN_OF_STREAM);
        assert_eq!(u64::from_le_bytes(bytes[6..14].try_into().unwrap()), 960);
        assert_eq!(
            u32::from_le_bytes(bytes[14..18].try_into().unwrap()),
            0x1234_5678
        );
        assert_eq!(
            u32::from_le_bytes(bytes[18..22].try_into().unwrap()),
            0,
            "first page"
        );
        assert_eq!(bytes[26], 1, "one segment");
        assert_eq!(bytes[27], 5, "the lacing value");
        assert_eq!(&bytes[28..], b"hello");

        let stored = u32::from_le_bytes(bytes[22..26].try_into().unwrap());
        let mut zeroed = bytes.clone();
        zeroed[22..26].fill(0);
        assert_eq!(
            stored,
            crc32(&zeroed),
            "the stored CRC must cover the whole page"
        );
    }
}
