//! A 4:2:0 video frame: a luma plane and two chroma planes.
//!
//! # Why the planes are separate
//!
//! ITU-T H.266 codes 4:2:0 content as three planes, and the chroma
//! planes are half the width and half the height of the luma plane.
//! GIRC follows that structure, so a GIRC frame carries the same three
//! planes a Y4M frame does.
//!
//! # What is different about the chroma planes
//!
//! Two things, both of them fixed by the standard rather than chosen
//! here.
//!
//! * **The transform type is not signalled for chroma.** H.266 clause
//!   7.3.9 codes `transform_type` for the luma plane only; the chroma
//!   transform type is inferred, and in 4:2:0 the inferred type is
//!   always the plain DCT-II. Fraunhofer HHI's reference encoder agrees:
//!   its chroma transform selection is gated on `m_useChromaTS`, which
//!   is off by default and only ever enabled for 4:4:4 content, and its
//!   intra search sets the chroma transform to `MTS_DCT2_DCT2`
//!   directly. The geometry-inference mechanism therefore applies to
//!   luma, which is exactly where H.266 signals a transform type at all.
//!
//! * **The quantization parameter is mapped, not equal.** The chroma
//!   quantization parameter comes from the luma one through H.266's
//!   chroma quantization parameter mapping, which the
//!   [`crate::chroma_qp`] module derives and checks against the
//!   reference encoder.

use crate::codec::Picture;

/// A 4:2:0 frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// The luma plane, at full resolution.
    pub luma: Picture,
    /// The blue-difference chroma plane, at half resolution.
    pub cb: Picture,
    /// The red-difference chroma plane, at half resolution.
    pub cr: Picture,
}

impl Frame {
    /// Build a frame from its three planes.
    pub fn new(luma: Picture, cb: Picture, cr: Picture) -> Self {
        Self { luma, cb, cr }
    }

    /// The width of the frame, in luma samples.
    pub fn width(&self) -> usize {
        self.luma.width
    }

    /// The height of the frame, in luma samples.
    pub fn height(&self) -> usize {
        self.luma.height
    }

    /// The two chroma planes, in the order the bitstream codes them.
    ///
    /// The order matters: the bitstream codes the blue-difference plane
    /// first, so the encoder and the decoder must agree on it.
    pub fn chroma_planes(&self) -> [&Picture; 2] {
        [&self.cb, &self.cr]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chroma planes must be half the luma plane's size in each
    /// dimension.
    ///
    /// That is the definition of 4:2:0, and a frame whose chroma planes
    /// were the wrong size would be coded against the wrong geometry.
    #[test]
    fn the_chroma_planes_are_half_resolution() {
        let frame = Frame::new(
            Picture::filled(32, 16, 0),
            Picture::filled(16, 8, 128),
            Picture::filled(16, 8, 128),
        );
        assert_eq!(frame.width(), 32);
        assert_eq!(frame.height(), 16);
        assert_eq!(frame.cb.width, 16);
        assert_eq!(frame.cb.height, 8);
        assert_eq!(frame.cr.width, 16);
        assert_eq!(frame.cr.height, 8);
    }

    /// The chroma planes must be handed out in the bitstream's order.
    #[test]
    fn the_chroma_planes_come_in_the_bitstream_order() {
        let frame = Frame::new(
            Picture::filled(16, 16, 0),
            Picture::filled(8, 8, 100),
            Picture::filled(8, 8, 200),
        );
        let planes = frame.chroma_planes();
        assert_eq!(
            planes[0].samples[0], 100,
            "the blue-difference plane comes first"
        );
        assert_eq!(
            planes[1].samples[0], 200,
            "the red-difference plane comes second"
        );
    }
}
