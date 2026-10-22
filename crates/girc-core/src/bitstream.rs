//! The GIRC bitstream: a byte-aligned container with a range-coded payload.
//!
//! # Container
//!
//! ```text
//! "GIRC"            4 bytes magic
//! version           u16, little endian
//! flags             u16, little endian
//! width             u32, little endian
//! height            u32, little endian
//! qp                u8
//! frames            u8
//! payload length    u32, little endian
//! payload           range-coded bytes
//! ```
//!
//! The payload holds the coded frames in order. The first is an intra
//! frame and the rest are inter frames, each predicted from the
//! reconstruction of the one before it.
//!
//! # Payload
//!
//! The payload carries the coded picture data. Syntax elements are coded
//! with a range coder using context-adaptive binary models, and the residual
//! coefficients use an exponential-Golomb-style binarisation over the same
//! coder. The coder itself is `constriction`'s range coder, a well-tested
//! implementation, so the bit accounting GIRC reports is the bit accounting
//! of a real entropy coder rather than an estimate.
//!
//! The bitstream is GIRC's own format. It is not VVC-conformant, and the
//! documentation says so: GIRC exists to measure one specific mechanism, and
//! its value lies in the measurement being exact and reproducible, not in
//! the bitstream interoperating with anything.

use constriction::backends::Cursor;
use constriction::stream::model::DefaultNonContiguousCategoricalDecoderModel;
use constriction::stream::model::DefaultNonContiguousCategoricalEncoderModel;
use constriction::stream::queue::DefaultRangeDecoder;
use constriction::stream::queue::DefaultRangeEncoder;
use constriction::stream::{Decode, Encode};

use crate::geometry::TransformSignalling;

/// The entropy coder's word type.
pub type Word = u32;

/// The fixed-point precision of the entropy models.
const PRECISION: u32 = 24;
/// One in the models' fixed-point representation.
const ONE: u32 = 1 << PRECISION;

/// The fixed-point probability of a one-bin for a context probability
/// expressed in `1/256` units.
fn probability_one(context_probability: u16) -> u32 {
    let p = (context_probability as u32) * ONE / 256;
    p.clamp(1, ONE - 1)
}

/// The magic bytes at the start of every GIRC bitstream.
pub const MAGIC: &[u8; 4] = b"GIRC";
/// The current bitstream version.
///
/// Version 2 codes the intra mode with four bits instead of three, so that
/// every mode in the ten-mode set is representable. A version 1 stream
/// would decode the last two modes wrongly, so the version is checked and
/// an old stream is rejected rather than mis-decoded.
pub const VERSION: u16 = 2;

/// The header of a GIRC bitstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitstreamHeader {
    pub version: u16,
    pub width: u32,
    pub height: u32,
    pub qp: u8,
    /// The number of frames in the payload.
    ///
    /// The first is coded as an intra frame and the rest as inter
    /// frames. The count is in the header so a decoder knows how many
    /// frames to expect before it starts decoding them.
    pub frames: u8,
    /// The signalling mode the payload uses.
    ///
    /// The mode has to be in the header rather than out of band: an
    /// explicit-signalling stream and an inferred-signalling stream have
    /// different syntax, so a decoder that guessed wrong would
    /// desynchronise. Recording it makes a stream self-describing.
    pub signalling: TransformSignalling,
}

/// The header flag that records the signalling mode.
const FLAG_INFERRED: u16 = 1;

impl BitstreamHeader {
    /// The size of the header in bytes, excluding the payload.
    pub const SIZE: usize = 4 + 2 + 2 + 4 + 4 + 1 + 1 + 4;

    /// Write a complete bitstream: the header followed by the payload.
    pub fn write(&self, out: &mut Vec<u8>, payload: &[Word]) {
        let flags = match self.signalling {
            TransformSignalling::Explicit => 0,
            TransformSignalling::Inferred => FLAG_INFERRED,
        };
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.push(self.qp);
        out.push(self.frames);
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        for word in payload {
            out.extend_from_slice(&word.to_le_bytes());
        }
    }

    /// Read a header from the start of a byte buffer, returning the header
    /// and the total number of bytes the bitstream occupies.
    pub fn read(bytes: &[u8]) -> Result<(Self, usize), BitstreamError> {
        if bytes.len() < Self::SIZE {
            return Err(BitstreamError::Truncated);
        }
        if &bytes[0..4] != MAGIC {
            return Err(BitstreamError::BadMagic);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION {
            return Err(BitstreamError::UnsupportedVersion(version));
        }
        let width = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let height = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let qp = bytes[16];
        let frames = bytes[17];
        let flags = u16::from_le_bytes([bytes[6], bytes[7]]);
        let signalling = if flags & FLAG_INFERRED != 0 {
            TransformSignalling::Inferred
        } else {
            TransformSignalling::Explicit
        };
        let payload_len = u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) as usize;
        Ok((
            Self {
                version,
                width,
                height,
                qp,
                frames,
                signalling,
            },
            Self::SIZE + payload_len * 4,
        ))
    }
}

/// Errors produced while reading a bitstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitstreamError {
    /// The buffer ended before the structure was complete.
    Truncated,
    /// The magic bytes did not match.
    BadMagic,
    /// The bitstream version is not understood.
    UnsupportedVersion(u16),
    /// The entropy-coded payload could not be decoded.
    Entropy(String),
}

impl std::fmt::Display for BitstreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BitstreamError::Truncated => write!(f, "bitstream is truncated"),
            BitstreamError::BadMagic => write!(f, "bitstream does not start with the GIRC magic"),
            BitstreamError::UnsupportedVersion(v) => {
                write!(f, "bitstream version {v} is not supported")
            }
            BitstreamError::Entropy(msg) => write!(f, "entropy decoding failed: {msg}"),
        }
    }
}

impl std::error::Error for BitstreamError {}

/// A context-adaptive binary arithmetic coder for the GIRC payload.
///
/// Each context is a pair of probabilities for the two possible bin values,
/// updated after every coded bin in the manner of ITU-T H.264 clause 9.3.2.1
/// so the coder adapts to the statistics of the data it sees.
#[derive(Debug, Clone)]
pub struct BinEncoder {
    coder: DefaultRangeEncoder<Vec<Word>>,
    /// Context state: the probability of a 1-bin, in `1/256` units.
    contexts: Vec<u16>,
    /// The number of contexts.
    num_contexts: usize,
}

impl BinEncoder {
    /// Create an encoder with the given number of contexts, all starting at
    /// the standard's initial probability of 0.5.
    pub fn new(num_contexts: usize) -> Self {
        Self {
            coder: DefaultRangeEncoder::new(),
            contexts: vec![128; num_contexts],
            num_contexts,
        }
    }

    /// Code a single bin in the given context.
    pub fn encode(&mut self, context: usize, bin: bool) {
        let p = self.contexts[context];
        let probability = probability_one(p);
        let model = DefaultNonContiguousCategoricalEncoderModel::from_symbols_and_nonzero_fixed_point_probabilities(
            [false, true],
            [ONE - probability, probability],
            false,
        )
        .expect("both bins have non-zero probability");
        self.coder
            .encode_symbol(bin, model)
            .expect("writing to a Vec cannot fail");

        // Adapt the context probability toward the coded value, in the
        // manner of ITU-T H.264 clause 9.3.2.1: the probability of the
        // symbol that was coded moves a sixteenth of the way toward
        // certainty.
        let updated = if bin {
            p + ((256 - p) >> 4)
        } else {
            p - (p >> 4)
        };
        self.contexts[context] = updated.clamp(1, 255);
    }

    /// Code a bin in bypass mode: no context, no adaptation.
    pub fn encode_bypass(&mut self, bin: bool) {
        let model = DefaultNonContiguousCategoricalEncoderModel::from_symbols_and_nonzero_fixed_point_probabilities(
            [false, true],
            [1u32 << 23, 1u32 << 23],
            false,
        )
        .expect("both bins have non-zero probability");
        self.coder
            .encode_symbol(bin, model)
            .expect("writing to a Vec cannot fail");
    }

    /// The number of bits written so far.
    pub fn bits_written(&self) -> usize {
        self.coder.num_bits()
    }

    /// The number of contexts in use.
    pub fn num_contexts(&self) -> usize {
        self.num_contexts
    }

    /// Finish the stream and return the compressed words.
    pub fn finish(self) -> Vec<Word> {
        self.coder
            .into_compressed()
            .expect("writing to a Vec cannot fail")
    }
}

/// The decoder counterpart of [`BinEncoder`].
#[derive(Debug, Clone)]
pub struct BinDecoder {
    coder: DefaultRangeDecoder<Cursor<Word, Vec<Word>>>,
    contexts: Vec<u16>,
}

impl BinDecoder {
    /// Create a decoder over the given compressed words.
    pub fn new(words: &[Word], num_contexts: usize) -> Result<Self, BitstreamError> {
        let coder = DefaultRangeDecoder::from_compressed(words.to_vec())
            .map_err(|e| BitstreamError::Entropy(e.to_string()))?;
        Ok(Self {
            coder,
            contexts: vec![128; num_contexts],
        })
    }

    /// Decode a single bin in the given context.
    pub fn decode(&mut self, context: usize) -> Result<bool, BitstreamError> {
        let p = self.contexts[context];
        let probability = probability_one(p);
        let model = DefaultNonContiguousCategoricalDecoderModel::from_symbols_and_nonzero_fixed_point_probabilities(
            [false, true],
            [ONE - probability, probability],
            false,
        )
        .expect("both bins have non-zero probability");
        let bin: bool = self
            .coder
            .decode_symbol(model)
            .map_err(|e| BitstreamError::Entropy(e.to_string()))?;

        // The context adapts the same way as the encoder's, so the two
        // stay in step.
        let updated = if bin {
            p + ((256 - p) >> 4)
        } else {
            p - (p >> 4)
        };
        self.contexts[context] = updated.clamp(1, 255);
        Ok(bin)
    }

    /// Decode a bin in bypass mode.
    pub fn decode_bypass(&mut self) -> Result<bool, BitstreamError> {
        let model = DefaultNonContiguousCategoricalDecoderModel::from_symbols_and_nonzero_fixed_point_probabilities(
            [false, true],
            [1u32 << 23, 1u32 << 23],
            false,
        )
        .expect("both bins have non-zero probability");
        self.coder
            .decode_symbol(model)
            .map_err(|e| BitstreamError::Entropy(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trips() {
        let header = BitstreamHeader {
            version: VERSION,
            width: 352,
            height: 288,
            qp: 27,
            frames: 3,
            signalling: TransformSignalling::Inferred,
        };
        let payload = vec![1u32, 2, 3, 4];
        let mut bytes = Vec::new();
        header.write(&mut bytes, &payload);
        let (parsed, consumed) = BitstreamHeader::read(&bytes).unwrap();
        assert_eq!(parsed, header);
        assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn bad_magic_is_rejected() {
        let mut bytes = vec![0u8; BitstreamHeader::SIZE];
        bytes[0] = b'X';
        assert_eq!(
            BitstreamHeader::read(&bytes).unwrap_err(),
            BitstreamError::BadMagic
        );
    }

    #[test]
    fn truncated_stream_is_rejected() {
        assert_eq!(
            BitstreamHeader::read(&[0u8; 4]).unwrap_err(),
            BitstreamError::Truncated
        );
    }

    #[test]
    fn bins_round_trip() {
        let mut encoder = BinEncoder::new(4);
        let bits = [
            true, false, true, true, false, false, true, false, true, true, true, false,
        ];
        for (i, bit) in bits.iter().enumerate() {
            encoder.encode(i % 4, *bit);
        }
        let compressed = encoder.finish();

        let mut decoder = BinDecoder::new(&compressed, 4).unwrap();
        for (i, bit) in bits.iter().enumerate() {
            assert_eq!(decoder.decode(i % 4).unwrap(), *bit, "bin {i}");
        }
    }

    #[test]
    fn bypass_bins_round_trip() {
        let mut encoder = BinEncoder::new(1);
        for bit in [true, false, true, true, false] {
            encoder.encode_bypass(bit);
        }
        let compressed = encoder.finish();
        let mut decoder = BinDecoder::new(&compressed, 1).unwrap();
        for expected in [true, false, true, true, false] {
            assert_eq!(decoder.decode_bypass().unwrap(), expected);
        }
    }

    /// The context probability must track the observed rate.
    ///
    /// An estimator that adapts by a fixed increment rather than a
    /// proportional one treats the probability as a signed count, so a
    /// context that sees a one-in-three rate drifts to the opposite
    /// extreme and every "one" bin becomes very expensive. The cost of
    /// coding a skewed stream must instead reflect its actual entropy.
    #[test]
    fn the_context_tracks_the_observed_rate() {
        let mut encoder = BinEncoder::new(1);
        let mut ones = 0u32;
        let total = 60_000u32;
        for i in 0..total {
            let bin = i % 3 == 0;
            ones += u32::from(bin);
            encoder.encode(0, bin);
        }
        let compressed = encoder.finish();
        // The entropy of a one-in-three source is about 0.918 bits per bin.
        let bits_per_bin = compressed.len() as f64 * 8.0 / total as f64;
        assert!(
            bits_per_bin < 1.05,
            "a one-in-three source cost {bits_per_bin} bits per bin"
        );
        assert!(ones > 0);
    }

    #[test]
    fn skewed_data_compresses() {
        // A long run of the same bin must cost far fewer bits than the
        // number of bins, which is the point of using a real entropy coder.
        let mut encoder = BinEncoder::new(1);
        for _ in 0..1000 {
            encoder.encode(0, false);
        }
        let compressed = encoder.finish();
        assert!(
            compressed.len() * 8 < 200,
            "1000 identical bins took {} bits",
            compressed.len() * 8
        );
    }
}
