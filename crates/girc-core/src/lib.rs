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
//! * [`inter`] -- inter prediction: motion vectors, motion compensation
//!   and the motion search.
//! * [`quant`] -- quantization.
//! * [`bitstream`] -- the container and the entropy coder.
//! * [`geometry`] -- the geometry-inference mechanism itself.
//! * [`chroma`] -- the 4:2:0 chroma planes.
//! * [`chroma_qp`] -- the chroma quantization parameter mapping.
//! * [`codec`] -- the encoder and decoder.
//! * [`yuv`] -- YUV4MPEG2 input and output.

pub mod bitstream;
pub mod chroma;
pub mod chroma_qp;
pub mod chroma_qp_oracle_vectors;
pub mod codec;
pub mod frame;
pub mod geometry;
pub mod inter;
pub mod intra;
pub mod quant;
pub mod transform;
pub mod transform_oracle_vectors;
pub mod transform_tables;
pub mod yuv;
