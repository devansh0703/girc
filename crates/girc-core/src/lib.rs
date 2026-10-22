//! GIRC: the Geometry-Inference Reference Codec.
//!
//! GIRC exists to measure one specific mechanism: whether a video codec can
//! *infer* a block's transform type from the block's local geometry instead
//! of signalling it. The codec is deliberately small and fully specified so
//! the measurement is exact and reproducible, and so the reference
//! implementation can be read, compiled and run by anyone evaluating the
//! idea.
//!
//! # Layout
//!
//! * [`transform`] -- the VVC integer transforms (DCT-II, DCT-VIII, DST-VII),
//!   bit-exact against Fraunhofer HHI's reference implementation.
//! * [`intra`] -- intra prediction.
//! * [`quant`] -- quantization.
//! * [`bitstream`] -- the container and the entropy coder.
//! * [`geometry`] -- the geometry-inference mechanism itself.
//! * [`codec`] -- the encoder and decoder.
//! * [`yuv`] -- YUV4MPEG2 input and output.

pub mod bitstream;
pub mod codec;
pub mod geometry;
pub mod intra;
pub mod quant;
pub mod transform;
pub mod transform_oracle_vectors;
pub mod transform_tables;
pub mod yuv;
