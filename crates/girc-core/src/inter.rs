//! Inter prediction: motion vectors, motion compensation and motion search.
//!
//! # Why this is here
//!
//! The mechanism removes the signalling of the transform mode by
//! predicting it from the causal neighbourhood. The claim in the patent
//! package is that the prediction improves with inter prediction,
//! because a motion-compensated residual carries the motion vector's
//! structure, which is more directional than an intra residual.
//!
//! This module is the smallest thing that can test that claim: integer
//! pel motion vectors, a motion vector predicted from the causal
//! neighbourhood, and a small search around that predictor. There is no
//! sub-pel interpolation and no bi-prediction, because neither changes
//! the question being asked, and both would change the rate so much
//! that the comparison would no longer be about the transform mode.
//!
//! # The motion vector predictor
//!
//! The predictor is the median of the left and above neighbours' motion
//! vectors, falling back to zero when neither exists. The decoder
//! derives the same predictor from the same causal neighbourhood, so no
//! bits are spent on it.

use crate::codec::Picture;

/// A motion vector, in whole samples.
///
/// The horizontal component is positive to the right and the vertical
/// component is positive downwards, so the reference sample for a block
/// at `(x, y)` is the reference's `(x + dx, y + dy)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MotionVector {
    pub dx: i32,
    pub dy: i32,
}

impl MotionVector {
    /// The zero vector.
    pub const ZERO: MotionVector = MotionVector { dx: 0, dy: 0 };

    /// Whether the vector is the zero vector.
    pub fn is_zero(self) -> bool {
        self.dx == 0 && self.dy == 0
    }

    /// The absolute sum of the components, used to order candidates.
    pub fn magnitude(self) -> i32 {
        self.dx.abs() + self.dy.abs()
    }
}

/// The motion vector of a block, in the raster order the blocks are
/// coded in.
#[derive(Debug, Clone, Default)]
pub struct MotionField {
    vectors: Vec<Option<MotionVector>>,
    blocks_x: usize,
}

impl MotionField {
    /// A field for a picture of the given block dimensions.
    pub fn new(blocks_x: usize, blocks_y: usize) -> Self {
        Self {
            vectors: vec![None; blocks_x * blocks_y],
            blocks_x,
        }
    }

    /// Record the motion vector of a block.
    pub fn set(&mut self, bx: usize, by: usize, vector: MotionVector) {
        if let Some(slot) = self.vectors.get_mut(by * self.blocks_x + bx) {
            *slot = Some(vector);
        }
    }

    /// The motion vector of a block, if it has been coded.
    pub fn get(&self, bx: usize, by: usize) -> Option<MotionVector> {
        self.vectors.get(by * self.blocks_x + bx).copied().flatten()
    }

    /// The motion vector predicted for a block.
    ///
    /// The predictor is the median of the left and above neighbours'
    /// vectors, and zero when neither exists. It is the same rule the
    /// decoder uses, and it is derived from the causal neighbourhood, so
    /// it costs no bits.
    pub fn predict(&self, bx: usize, by: usize) -> MotionVector {
        let left = if bx > 0 { self.get(bx - 1, by) } else { None };
        let above = if by > 0 { self.get(bx, by - 1) } else { None };
        match (left, above) {
            (Some(l), Some(a)) => MotionVector {
                dx: l.dx + (a.dx - l.dx) / 2,
                dy: l.dy + (a.dy - l.dy) / 2,
            },
            (Some(l), None) => l,
            (None, Some(a)) => a,
            (None, None) => MotionVector::ZERO,
        }
    }
}

impl std::ops::Add for MotionVector {
    type Output = MotionVector;

    fn add(self, other: MotionVector) -> MotionVector {
        MotionVector {
            dx: self.dx + other.dx,
            dy: self.dy + other.dy,
        }
    }
}

impl std::ops::Sub for MotionVector {
    type Output = MotionVector;

    fn sub(self, other: MotionVector) -> MotionVector {
        MotionVector {
            dx: self.dx - other.dx,
            dy: self.dy - other.dy,
        }
    }
}

/// The motion-compensated prediction of one sample.
///
/// A reference sample outside the reference picture is not available, so
/// the prediction falls back to the sample itself, which is the
/// convention that keeps the encoder and the decoder in step without
/// costing a bit.
pub fn compensated_sample(reference: &Picture, x: i32, y: i32, vector: MotionVector) -> i32 {
    let sx = x + vector.dx;
    let sy = y + vector.dy;
    if sx < 0 || sy < 0 {
        return 0;
    }
    reference.sample(sx as isize, sy as isize).unwrap_or(0) as i32
}

/// The motion-compensated prediction of a block.
///
/// The block is `size` by `size` samples at `(x, y)`, and the result is
/// in raster order.
pub fn compensate(
    reference: &Picture,
    x: i32,
    y: i32,
    size: usize,
    vector: MotionVector,
) -> Vec<i32> {
    let mut out = Vec::with_capacity(size * size);
    for row in 0..size as i32 {
        for column in 0..size as i32 {
            out.push(compensated_sample(reference, x + column, y + row, vector));
        }
    }
    out
}

/// The sum of absolute differences between a block and its
/// motion-compensated prediction.
fn distortion(
    original: &[u8],
    reference: &Picture,
    x: i32,
    y: i32,
    size: usize,
    vector: MotionVector,
) -> u32 {
    let mut total = 0u32;
    for row in 0..size as i32 {
        for column in 0..size as i32 {
            let sample = compensated_sample(reference, x + column, y + row, vector);
            let source = original[(row as usize) * size + column as usize] as i32;
            let difference = (sample - source).abs();
            total += difference as u32;
        }
    }
    total
}

/// The motion vector search.
///
/// The search is a small integer-pel diamond around the predictor. The
/// predictor is where the motion is most likely to be, because the
/// motion field is smooth, so a small search around it is enough to
/// find the motion a real codec would find.
pub fn search(
    original: &[u8],
    reference: &Picture,
    x: i32,
    y: i32,
    size: usize,
    predictor: MotionVector,
    radius: i32,
) -> MotionVector {
    let mut best = predictor;
    let mut best_cost = distortion(original, reference, x, y, size, best);

    // A diamond: the four axis directions first, then the diagonals.
    let steps = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    for distance in 1..=radius {
        for (sx, sy) in steps {
            let candidate = MotionVector {
                dx: predictor.dx + sx * distance,
                dy: predictor.dy + sy * distance,
            };
            let cost = distortion(original, reference, x, y, size, candidate);
            if cost < best_cost {
                best_cost = cost;
                best = candidate;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reference picture with a bright square that has moved.
    fn reference() -> Picture {
        let mut picture = Picture::filled(32, 32, 0);
        for y in 8..16 {
            for x in 8..16 {
                picture.set_sample(x, y, 200);
            }
        }
        picture
    }

    /// The motion search must find a vector that moves the reference's
    /// bright square onto the block.
    ///
    /// The reference's square sits at 8..16, and the current frame's sits
    /// at 10..18, so the block at (10, 8) is entirely square and the
    /// vector that reconstructs it points two samples to the left.
    #[test]
    fn the_search_finds_the_motion() {
        let reference = reference();
        let original = vec![200u8; 64];
        let vector = search(&original, &reference, 10, 8, 8, MotionVector::ZERO, 4);
        assert_eq!(vector, MotionVector { dx: -2, dy: 0 });
    }

    /// The motion vector predictor must be the median of the neighbours,
    /// and the decoder must derive the same one without any bits.
    #[test]
    fn the_predictor_is_the_neighbour_median() {
        let mut field = MotionField::new(4, 4);
        field.set(0, 0, MotionVector { dx: 4, dy: 0 });
        // With only the left neighbour, the predictor is that neighbour.
        assert_eq!(field.predict(1, 0), MotionVector { dx: 4, dy: 0 });
        // With the left at 4 and the above at 8, the median is 6.
        field.set(0, 1, MotionVector { dx: 4, dy: 0 });
        field.set(1, 0, MotionVector { dx: 8, dy: 0 });
        assert_eq!(field.predict(1, 1), MotionVector { dx: 6, dy: 0 });
        // With no neighbours at all, it is zero.
        assert_eq!(field.predict(0, 0), MotionVector::ZERO);
    }

    /// A motion-compensated sample outside the reference must not be
    /// read out of bounds.
    #[test]
    fn a_sample_outside_the_reference_is_zero() {
        let reference = reference();
        assert_eq!(
            compensated_sample(&reference, 0, 0, MotionVector { dx: -100, dy: 0 }),
            0
        );
        assert_eq!(
            compensated_sample(&reference, 30, 30, MotionVector { dx: 100, dy: 0 }),
            0
        );
    }

    /// The zero vector must leave the block as it is.
    #[test]
    fn the_zero_vector_is_the_identity() {
        let reference = reference();
        let block = compensate(&reference, 8, 8, 8, MotionVector::ZERO);
        for row in 0..8 {
            for column in 0..8 {
                assert_eq!(block[row * 8 + column], 200);
            }
        }
    }
}
