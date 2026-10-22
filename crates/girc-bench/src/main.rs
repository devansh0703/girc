//! The GIRC benchmark harness.
//!
//! This is the tool that produces the evidence the patent package rests on.
//! It runs GIRC in both signalling modes across a quantization parameter
//! sweep, measures rate and distortion against the source, and computes the
//! Bjontegaard delta rate between the two modes.
//!
//! It also runs the anchor codecs -- x265 (HEVC), SVT-AV1 (AV1) and VVenC
//! (VVC) -- on the same content, so the report places GIRC's absolute
//! efficiency in context against the codecs a codec team would actually
//! compare against.
//!
//! # What the report can and cannot claim
//!
//! The primary number is the BD-rate between GIRC-explicit-signalling and
//! GIRC-inferred-signalling. That comparison is internally controlled: the
//! two codecs differ only in how the transform type reaches the decoder, so
//! the difference is attributable to the mechanism.
//!
//! The anchor numbers are context, not a claim of competitiveness. GIRC is
//! an intra-only research codec with a small mode set; it is not a VVC
//! competitor, and the report says so.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use girc_core::codec::{encode, EncoderConfig};
use girc_core::geometry::{TransformPredictor, TransformSignalling};
use girc_core::yuv::Yuv4Frame;
use serde::{Deserialize, Serialize};

/// A single rate-distortion point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatePoint {
    pub qp: i32,
    /// The coded size in bits.
    pub bits: u64,
    /// The peak signal-to-noise ratio in decibels.
    pub psnr_db: f64,
}

/// The result for one codec on one sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecResult {
    pub codec: String,
    pub sequence: String,
    pub width: usize,
    pub height: usize,
    pub points: Vec<RatePoint>,
    /// The number of blocks whose transform differed from the decoder's
    /// prediction, when the codec is GIRC in inferred mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prediction_mismatches: Option<u64>,
    /// The number of blocks coded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_count: Option<u64>,
    /// The blocks whose transform differed from the decoder's
    /// prediction, split by whether the block was intra or inter
    /// predicted.
    ///
    /// The split is what the inter hypothesis is about: an inter
    /// residual is more structured, so the transform should be more
    /// predictable, and the mismatch rate should be lower for the
    /// inter blocks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intra_mismatches: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inter_mismatches: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intra_blocks: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inter_blocks: Option<u64>,
}

impl CodecResult {
    /// The mean PSNR over the sweep.
    pub fn mean_psnr(&self) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }
        self.points.iter().map(|p| p.psnr_db).sum::<f64>() / self.points.len() as f64
    }

    /// The total coded size over the sweep.
    pub fn total_bits(&self) -> u64 {
        self.points.iter().map(|p| p.bits).sum()
    }
}

/// The Bjontegaard delta rate between two rate-distortion curves.
///
/// The method is the standard cubic-polynomial interpolation of the
/// Bjontegaard delta rate, as described in ITU-T-T VCEG-M11 and used by the
/// JVET common test conditions. `reference` is the anchor curve and `test`
/// the curve being measured; a negative result means the test codec needs
/// fewer bits at the same quality.
///
/// The interpolation is over log-bitrate against PSNR, and the integration
/// runs over the overlapping PSNR range of the two curves.
pub fn bd_rate(reference: &[RatePoint], test: &[RatePoint]) -> Option<f64> {
    if reference.len() < 4 || test.len() < 4 {
        return None;
    }

    // A rate-distortion sweep normally runs from low quality to high, so
    // the points are ordered by PSNR before the fit rather than in the
    // order they were measured in.
    let mut ref_points: Vec<&RatePoint> = reference.iter().collect();
    let mut test_points: Vec<&RatePoint> = test.iter().collect();
    ref_points.sort_by(|a, b| a.psnr_db.partial_cmp(&b.psnr_db).unwrap());
    test_points.sort_by(|a, b| a.psnr_db.partial_cmp(&b.psnr_db).unwrap());

    // The two curves must be monotone in PSNR for the fit to be meaningful.
    let ref_psnr: Vec<f64> = ref_points.iter().map(|p| p.psnr_db).collect();
    let test_psnr: Vec<f64> = test_points.iter().map(|p| p.psnr_db).collect();
    if !ref_psnr.windows(2).all(|w| w[1] > w[0]) || !test_psnr.windows(2).all(|w| w[1] > w[0]) {
        return None;
    }

    let ref_log: Vec<f64> = ref_points.iter().map(|p| (p.bits as f64).ln()).collect();
    let test_log: Vec<f64> = test_points.iter().map(|p| (p.bits as f64).ln()).collect();

    // Fit log-bitrate as a cubic polynomial in PSNR for each curve, over the
    // overlapping PSNR range.
    let low = ref_psnr[0].max(test_psnr[0]);
    let high = ref_psnr[ref_psnr.len() - 1].min(test_psnr[test_psnr.len() - 1]);
    if high <= low {
        return None;
    }

    let ref_poly = CubicFit::fit(&ref_psnr, &ref_log)?;
    let test_poly = CubicFit::fit(&test_psnr, &test_log)?;

    // The integral of the difference of the two polynomials over the
    // overlapping range, divided by the range, is the average log-bitrate
    // difference; exponentiating gives the BD-rate.
    let samples = 1024;
    let step = (high - low) / samples as f64;
    let mut total = 0.0;
    for i in 0..=samples {
        let psnr = low + step * i as f64;
        total += (test_poly.eval(psnr) - ref_poly.eval(psnr)) * step;
    }
    let mean_log_diff = total / (high - low);
    Some((mean_log_diff.exp() - 1.0) * 100.0)
}

/// A cubic polynomial fit by least squares, solved with Gaussian elimination.
pub struct CubicFit {
    coefficients: [f64; 4],
}

impl CubicFit {
    /// Fit `y = c0 + c1 x + c2 x^2 + c3 x^3` to the given points.
    pub fn fit(x: &[f64], y: &[f64]) -> Option<Self> {
        let n = x.len();
        if n < 4 || n != y.len() {
            return None;
        }

        // Normal equations: A c = b, with A[i][j] = sum(x^k * x^j).
        let mut a = [[0f64; 4]; 4];
        let mut b = [0f64; 4];
        for i in 0..n {
            let powers = [1.0, x[i], x[i] * x[i], x[i] * x[i] * x[i]];
            for j in 0..4 {
                b[j] += powers[j] * y[i];
                for k in 0..4 {
                    a[j][k] += powers[j] * powers[k];
                }
            }
        }

        let solution = solve(&mut a, &b)?;
        Some(Self {
            coefficients: solution,
        })
    }

    /// Evaluate the polynomial at `x`.
    pub fn eval(&self, x: f64) -> f64 {
        let c = &self.coefficients;
        c[0] + x * (c[1] + x * (c[2] + x * c[3]))
    }
}

/// Solve a 4x4 linear system by Gaussian elimination with partial pivoting.
fn solve(a: &mut [[f64; 4]; 4], b: &[f64; 4]) -> Option<[f64; 4]> {
    let mut m = *a;
    let mut rhs = *b;

    for col in 0..4 {
        // Partial pivoting.
        let mut pivot = col;
        for row in (col + 1)..4 {
            if m[row][col].abs() > m[pivot][col].abs() {
                pivot = row;
            }
        }
        if m[pivot][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, pivot);
        rhs.swap(col, pivot);

        for row in (col + 1)..4 {
            let factor = m[row][col] / m[col][col];
            let pivot_row = m[col];
            for (k, cell) in m[row].iter_mut().enumerate().skip(col) {
                *cell -= factor * pivot_row[k];
            }
            rhs[row] -= factor * rhs[col];
        }
    }

    let mut solution = [0f64; 4];
    for row in (0..4).rev() {
        let mut sum = rhs[row];
        for k in (row + 1)..4 {
            sum -= m[row][k] * solution[k];
        }
        solution[row] = sum / m[row][row];
    }
    Some(solution)
}

/// The peak signal-to-noise ratio between two luma planes.
/// The mean squared error between two planes.
pub fn mse(reference: &[u8], test: &[u8]) -> f64 {
    if reference.len() != test.len() || reference.is_empty() {
        return 0.0;
    }
    let total: f64 = reference
        .iter()
        .zip(test.iter())
        .map(|(a, b)| {
            let difference = *a as f64 - *b as f64;
            difference * difference
        })
        .sum();
    total / reference.len() as f64
}

/// The peak signal-to-noise ratio from a mean squared error.
pub fn mse_to_psnr(mse: f64) -> f64 {
    if mse <= 0.0 {
        return 100.0;
    }
    10.0 * (255.0f64 * 255.0 / mse).log10()
}

pub fn psnr(reference: &[u8], test: &[u8]) -> f64 {
    assert_eq!(reference.len(), test.len());
    if reference.is_empty() {
        return 0.0;
    }
    let mut sum = 0f64;
    for (a, b) in reference.iter().zip(test.iter()) {
        let d = *a as f64 - *b as f64;
        sum += d * d;
    }
    let mse = sum / reference.len() as f64;
    if mse <= 0.0 {
        return f64::INFINITY;
    }
    10.0 * (255.0 * 255.0 / mse).log10()
}

/// The anchor codecs the benchmark can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// GIRC with explicit transform-type signalling.
    GircExplicit,
    /// GIRC with geometry-inference transform signalling.
    GircInferred,
    /// x265, the HEVC encoder.
    X265,
    /// SVT-AV1, the AV1 encoder.
    SvtAv1,
    /// VVenC, the VVC encoder.
    VVenC,
}

impl Anchor {
    /// The display name of the anchor.
    pub fn name(self) -> &'static str {
        match self {
            Anchor::GircExplicit => "girc-explicit",
            Anchor::GircInferred => "girc-inferred",
            Anchor::X265 => "x265",
            Anchor::SvtAv1 => "svt-av1",
            Anchor::VVenC => "vvenc",
        }
    }
}

/// The quantization parameters the benchmark sweeps.
pub const DEFAULT_QPS: [i32; 4] = [22, 27, 32, 37];

/// Run GIRC over a quantization parameter sweep and return its rate-
/// distortion points.
pub fn run_girc(
    sequence: &Path,
    qps: &[i32],
    signalling: TransformSignalling,
    predictor: &TransformPredictor,
    transform_margin: f32,
    frames: usize,
) -> Result<CodecResult> {
    let read = Yuv4Frame::read_many(sequence, frames.max(1))?;
    let pictures: Vec<_> = read.iter().map(|f| f.luma_picture()).collect();
    let picture = &pictures[0];
    let mut points = Vec::new();
    let mut mismatches = 0u64;
    let mut blocks = 0u64;
    let mut intra_mismatches = 0u64;
    let mut inter_mismatches = 0u64;
    let mut intra_blocks = 0u64;
    let mut inter_blocks = 0u64;

    for qp in qps {
        let config = EncoderConfig {
            qp: *qp,
            signalling,
            predictor: predictor.clone(),
            transform_margin,
            ..Default::default()
        };
        let result = encode(&pictures, &config)?;

        // The distortion is the mean over the frames, so the curve is
        // comparable with the anchors', which also code every frame.
        let mut total_mse = 0f64;
        for (original, reconstruction) in pictures.iter().zip(result.reconstructions.iter()) {
            total_mse += mse(&original.samples, &reconstruction.samples);
        }
        let psnr = mse_to_psnr(total_mse / pictures.len() as f64);

        points.push(RatePoint {
            qp: *qp,
            bits: (result.bitstream.len() * 8) as u64,
            psnr_db: psnr,
        });
        mismatches += result.prediction_mismatches as u64;
        blocks += result.block_count as u64;
        intra_mismatches += result.intra_mismatches as u64;
        inter_mismatches += result.inter_mismatches as u64;
        intra_blocks += result.intra_blocks as u64;
        inter_blocks += result.inter_blocks as u64;
    }

    Ok(CodecResult {
        codec: match signalling {
            TransformSignalling::Explicit => Anchor::GircExplicit.name(),
            TransformSignalling::Inferred => Anchor::GircInferred.name(),
        }
        .to_string(),
        sequence: sequence
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        width: picture.width,
        height: picture.height,
        points,
        prediction_mismatches: Some(mismatches),
        block_count: Some(blocks),
        intra_mismatches: Some(intra_mismatches),
        inter_mismatches: Some(inter_mismatches),
        intra_blocks: Some(intra_blocks),
        inter_blocks: Some(inter_blocks),
    })
}

/// Run an external anchor encoder over a quantization parameter sweep.
///
/// The encoder is invoked through its command-line interface, which is how a
/// codec team would run it. `qp_to_args` maps a GIRC quantization parameter
/// onto the anchor's own rate-control parameter.
pub fn run_anchor(
    anchor: Anchor,
    sequence: &Path,
    qps: &[i32],
    work_dir: &Path,
    frames: usize,
) -> Result<CodecResult> {
    let read = Yuv4Frame::read_many(sequence, frames.max(1))?;
    let mut points = Vec::new();

    for qp in qps {
        let output = work_dir.join(format!(
            "{}-{qp}.out",
            sequence.file_stem().unwrap_or_default().to_string_lossy()
        ));
        run_anchor_command(anchor, sequence, *qp, &output, frames.max(1))?;
        let bits = file_size_bits(&output)?;
        let reconstruction =
            decode_with_anchor(anchor, sequence, &output, work_dir, *qp, frames.max(1))?;
        // The distortion is the mean over the frames, the same way the
        // GIRC curve is computed.
        let mut total_mse = 0f64;
        for (original, decoded) in read.iter().zip(reconstruction.iter()) {
            total_mse += mse(&original.y, decoded);
        }
        let psnr = mse_to_psnr(total_mse / read.len() as f64);
        points.push(RatePoint {
            qp: *qp,
            bits,
            psnr_db: psnr,
        });
        let _ = std::fs::remove_file(&output);
    }

    Ok(CodecResult {
        codec: anchor.name().to_string(),
        sequence: sequence
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        width: read[0].width,
        height: read[0].height,
        points,
        prediction_mismatches: None,
        block_count: None,
        intra_mismatches: None,
        inter_mismatches: None,
        intra_blocks: None,
        inter_blocks: None,
    })
}

/// The size of a file in bits.
fn file_size_bits(path: &Path) -> Result<u64> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("reading the size of {}", path.display()))?;
    Ok(metadata.len() * 8)
}

/// Run the anchor encoder's command line.
fn run_anchor_command(
    anchor: Anchor,
    sequence: &Path,
    qp: i32,
    output: &Path,
    frames: usize,
) -> Result<()> {
    let frames = frames.to_string();
    let status = match anchor {
        Anchor::X265 => Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(sequence)
            .args([
                "-frames:v",
                &frames,
                "-c:v",
                "libx265",
                "-preset",
                "medium",
                "-qp",
                &qp.to_string(),
            ])
            .args(["-pix_fmt", "yuv420p", "-f", "hevc"])
            .arg(output)
            .status(),
        Anchor::SvtAv1 => Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(sequence)
            .args([
                "-frames:v",
                &frames,
                "-c:v",
                "libsvtav1",
                "-preset",
                "6",
                "-crf",
                &qp.to_string(),
                "-pix_fmt",
                "yuv420p",
            ])
            .args(["-f", "ivf"])
            .arg(output)
            .status(),
        Anchor::VVenC => Command::new("vvencapp")
            .args(["--input"])
            .arg(sequence)
            .args(["--output"])
            .arg(output)
            .args(["--preset", "medium", "-q", &qp.to_string()])
            .args(["--frames", &frames])
            .status(),
        Anchor::GircExplicit | Anchor::GircInferred => {
            bail!("GIRC anchors are run in-process, not through a command line")
        }
    };

    let status = status.with_context(|| format!("running {}", anchor.name()))?;
    if !status.success() {
        bail!("{} failed with status {status}", anchor.name());
    }
    Ok(())
}

/// Decode an anchor's output back to a luma plane.
fn decode_with_anchor(
    anchor: Anchor,
    sequence: &Path,
    encoded: &Path,
    work_dir: &Path,
    qp: i32,
    frames: usize,
) -> Result<Vec<Vec<u8>>> {
    let output = work_dir.join(format!(
        "{}-{qp}.decoded.y4m",
        sequence.file_stem().unwrap_or_default().to_string_lossy()
    ));
    let frame_count = frames;
    let frames = frames.to_string();
    let status = match anchor {
        Anchor::X265 => Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(encoded)
            .args([
                "-frames:v",
                &frames,
                "-pix_fmt",
                "yuv420p",
                "-f",
                "yuv4mpegpipe",
            ])
            .arg(&output)
            .status(),
        Anchor::SvtAv1 => Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(encoded)
            .args([
                "-frames:v",
                &frames,
                "-pix_fmt",
                "yuv420p",
                "-f",
                "yuv4mpegpipe",
            ])
            .arg(&output)
            .status(),
        Anchor::VVenC => {
            // VVenC is an encoder only. Its bitstream is decoded with
            // FFmpeg, exactly as a third party would decode it.
            let bitstream = work_dir.join(format!(
                "{}-{qp}.266",
                sequence.file_stem().unwrap_or_default().to_string_lossy()
            ));
            let encoded = Command::new("vvencapp")
                .args(["--input"])
                .arg(sequence)
                .args(["--output"])
                .arg(&bitstream)
                .args(["--preset", "medium", "-q", &qp.to_string()])
                .args(["--frames", &frames])
                .status();
            let encoded = encoded.with_context(|| format!("decoding with {}", anchor.name()))?;
            if !encoded.success() {
                bail!(
                    "decoding with {} failed with status {encoded}",
                    anchor.name()
                );
            }
            Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(&bitstream)
                .args([
                    "-frames:v",
                    &frames,
                    "-pix_fmt",
                    "yuv420p",
                    "-f",
                    "yuv4mpegpipe",
                ])
                .arg(&output)
                .status()
        }
        Anchor::GircExplicit | Anchor::GircInferred => {
            bail!("GIRC anchors are decoded in-process")
        }
    };
    let status = status.with_context(|| format!("decoding with {}", anchor.name()))?;
    if !status.success() {
        bail!(
            "decoding with {} failed with status {status}",
            anchor.name()
        );
    }
    let decoded = Yuv4Frame::read_many(&output, frame_count)?;
    let _ = std::fs::remove_file(&output);
    Ok(decoded.into_iter().map(|f| f.y).collect())
}

/// The benchmark command-line arguments.
#[derive(Parser, Debug)]
#[command(name = "girc-bench", about = "GIRC benchmark harness", version)]
struct Args {
    /// The Y4M sequences to benchmark.
    #[arg(long, required = true)]
    sequence: Vec<PathBuf>,
    /// The quantization parameters to sweep.
    #[arg(long, value_delimiter = ',')]
    qps: Option<Vec<i32>>,
    /// How many frames of each sequence to code.
    ///
    /// The first frame is coded as an intra frame and the rest as inter
    /// frames, each predicted from the reconstruction of the one before
    /// it. The anchors are given the same number of frames, so the
    /// comparison is frame-count matched.
    #[arg(long, default_value_t = 1)]
    frames: usize,
    /// A trained predictor model for the inferred mode.
    #[arg(long)]
    model: Option<PathBuf>,
    /// The rate-distortion margin within which the encoder accepts the
    /// decoder's predicted transform.
    #[arg(long, default_value_t = 0.0)]
    margin: f32,
    /// The external encoders to run, by name.
    #[arg(long, value_delimiter = ',', default_value = "x265,svt-av1,vvenc")]
    anchors: Vec<String>,
    /// Where to write the report.
    #[arg(long, default_value = "bench-out/report.json")]
    report: PathBuf,
    /// A directory for intermediate files.
    #[arg(long, default_value = "bench-out/work")]
    work_dir: PathBuf,
    /// Sweep the transform margin and report the trade-off curve.
    ///
    /// The margin is the rate the encoder is willing to give up to avoid
    /// signalling the transform type. Sweeping it shows the whole
    /// rate-distortion trade the mechanism offers, rather than a single
    /// operating point.
    #[arg(long, value_delimiter = ',')]
    margin_sweep: Option<Vec<f32>>,
}

/// The full benchmark report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// The report format version.
    pub version: u32,
    /// The time the report was produced, in UTC.
    pub generated_utc: String,
    /// The tool versions the report was produced with.
    pub tool_versions: BTreeMap<String, String>,
    /// The results, one per codec per sequence.
    pub results: Vec<CodecResult>,
    /// The BD-rate of each codec against the GIRC-explicit baseline.
    pub bd_rate_vs_baseline: BTreeMap<String, BTreeMap<String, f64>>,
    /// The BD-rate of GIRC-inferred against GIRC-explicit.
    pub mechanism_bd_rate: BTreeMap<String, f64>,
    /// The margin sweep, when one was run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_sweep: Option<Vec<MarginPoint>>,
}

/// One point on the margin sweep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarginPoint {
    /// The margin, in bits of rate.
    pub margin: f32,
    /// The BD-rate of the inferred mode against the explicit mode, per
    /// sequence.
    pub bd_rate: BTreeMap<String, f64>,
    /// The mean BD-rate across the sequences.
    pub mean_bd_rate: f64,
    /// The fraction of blocks where the decoder's prediction was wrong.
    pub mean_mismatch_rate: f64,
    /// The same rate, for the intra blocks alone.
    pub mean_intra_mismatch_rate: f64,
    /// The same rate, for the inter blocks alone.
    pub mean_inter_mismatch_rate: f64,
    /// The mean whole-frame encode time of the two modes, in seconds.
    pub mean_encode_seconds: f64,
}

/// Run the margin sweep.
///
/// The sweep answers the question the mechanism poses: how much rate has
/// to be given up before the saving in transform-type signalling is worth
/// more than the distortion it costs.
fn run_margin_sweep(
    sequences: &[PathBuf],
    qps: &[i32],
    predictor: &TransformPredictor,
    margins: &[f32],
    frames: usize,
) -> Result<Vec<MarginPoint>> {
    let mut points = Vec::new();

    for margin in margins {
        let mut per_sequence = BTreeMap::new();
        let mut rates = Vec::new();
        let mut mismatch_rates = Vec::new();
        let mut intra_mismatch_rates = Vec::new();
        let mut inter_mismatch_rates = Vec::new();
        let mut times = Vec::new();

        for sequence in sequences {
            let name = sequence
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();

            let mut explicit = Vec::new();
            let mut inferred = Vec::new();
            let mut mismatches = 0u64;
            let mut blocks = 0u64;
            let mut intra_mismatches = 0u64;
            let mut inter_mismatches = 0u64;
            let mut intra_blocks = 0u64;
            let mut inter_blocks = 0u64;
            let mut seconds = 0.0;

            for signalling in [TransformSignalling::Explicit, TransformSignalling::Inferred] {
                let read = Yuv4Frame::read_many(sequence, frames.max(1))?;
                let pictures: Vec<_> = read.iter().map(|f| f.luma_picture()).collect();
                let mut points_for_mode = Vec::new();
                for qp in qps {
                    let config = EncoderConfig {
                        qp: *qp,
                        signalling,
                        predictor: predictor.clone(),
                        transform_margin: *margin,
                        ..Default::default()
                    };
                    let start = std::time::Instant::now();
                    let result = encode(&pictures, &config)?;
                    seconds += start.elapsed().as_secs_f64();
                    let mut total_mse = 0f64;
                    for (original, reconstruction) in
                        pictures.iter().zip(result.reconstructions.iter())
                    {
                        total_mse += mse(&original.samples, &reconstruction.samples);
                    }
                    points_for_mode.push(RatePoint {
                        qp: *qp,
                        bits: (result.bitstream.len() * 8) as u64,
                        psnr_db: mse_to_psnr(total_mse / pictures.len() as f64),
                    });
                    if signalling == TransformSignalling::Inferred {
                        mismatches += result.prediction_mismatches as u64;
                        blocks += result.block_count as u64;
                        intra_mismatches += result.intra_mismatches as u64;
                        inter_mismatches += result.inter_mismatches as u64;
                        intra_blocks += result.intra_blocks as u64;
                        inter_blocks += result.inter_blocks as u64;
                    }
                }
                if signalling == TransformSignalling::Explicit {
                    explicit = points_for_mode;
                } else {
                    inferred = points_for_mode;
                }
            }

            if let Some(rate) = bd_rate(&explicit, &inferred) {
                per_sequence.insert(name.clone(), rate);
                rates.push(rate);
            }
            if blocks > 0 {
                mismatch_rates.push(mismatches as f64 / blocks as f64);
            }
            if intra_blocks > 0 {
                intra_mismatch_rates.push(intra_mismatches as f64 / intra_blocks as f64);
            }
            if inter_blocks > 0 {
                inter_mismatch_rates.push(inter_mismatches as f64 / inter_blocks as f64);
            }
            times.push(seconds);
        }

        points.push(MarginPoint {
            margin: *margin,
            mean_bd_rate: if rates.is_empty() {
                f64::NAN
            } else {
                rates.iter().sum::<f64>() / rates.len() as f64
            },
            bd_rate: per_sequence,
            mean_mismatch_rate: if mismatch_rates.is_empty() {
                f64::NAN
            } else {
                mismatch_rates.iter().sum::<f64>() / mismatch_rates.len() as f64
            },
            mean_intra_mismatch_rate: if intra_mismatch_rates.is_empty() {
                f64::NAN
            } else {
                intra_mismatch_rates.iter().sum::<f64>() / intra_mismatch_rates.len() as f64
            },
            mean_inter_mismatch_rate: if inter_mismatch_rates.is_empty() {
                f64::NAN
            } else {
                inter_mismatch_rates.iter().sum::<f64>() / inter_mismatch_rates.len() as f64
            },
            mean_encode_seconds: if times.is_empty() {
                f64::NAN
            } else {
                times.iter().sum::<f64>() / times.len() as f64
            },
        });

        eprintln!(
            "margin {:>10}: mean BD-rate {:+.3}%, mismatch {:+.1}%",
            points.last().unwrap().margin,
            points.last().unwrap().mean_bd_rate,
            points.last().unwrap().mean_mismatch_rate * 100.0
        );
    }

    Ok(points)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let qps = args.qps.unwrap_or_else(|| DEFAULT_QPS.to_vec());
    std::fs::create_dir_all(&args.work_dir)?;
    if let Some(parent) = args.report.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let predictor = match &args.model {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("reading the model at {}", path.display()))?;
            TransformPredictor::from_json(&text).map_err(|e| anyhow!("bad model: {e}"))?
        }
        None => TransformPredictor::constant_dct2(),
    };

    let mut results = Vec::new();
    for sequence in &args.sequence {
        eprintln!("sequence: {}", sequence.display());
        results.push(run_girc(
            sequence,
            &qps,
            TransformSignalling::Explicit,
            &predictor,
            args.margin,
            args.frames,
        )?);
        results.push(run_girc(
            sequence,
            &qps,
            TransformSignalling::Inferred,
            &predictor,
            args.margin,
            args.frames,
        )?);

        for anchor_name in &args.anchors {
            // An empty entry means no anchor; the flag accepts a list, and
            // an empty list is a way to run GIRC on its own.
            if anchor_name.trim().is_empty() {
                continue;
            }
            let anchor = match anchor_name.as_str() {
                "x265" => Anchor::X265,
                "svt-av1" => Anchor::SvtAv1,
                "vvenc" => Anchor::VVenC,
                other => bail!("unknown anchor encoder {other}"),
            };
            match run_anchor(anchor, sequence, &qps, &args.work_dir, args.frames) {
                Ok(result) => results.push(result),
                Err(error) => {
                    // A missing anchor must not sink the whole report; it is
                    // recorded and the run continues.
                    eprintln!("  warning: {} unavailable: {error:#}", anchor.name());
                }
            }
        }
    }

    // The BD-rate of every codec against the GIRC-explicit baseline, and the
    // mechanism's own BD-rate.
    let mut bd_rate_vs_baseline: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    let mut mechanism_bd_rate: BTreeMap<String, f64> = BTreeMap::new();

    for sequence in &args.sequence {
        let name = sequence
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let baseline = results
            .iter()
            .find(|r| r.codec == Anchor::GircExplicit.name() && r.sequence == name);
        let Some(baseline) = baseline else {
            continue;
        };

        let mut per_codec = BTreeMap::new();
        for result in results.iter().filter(|r| r.sequence == name) {
            if result.codec == Anchor::GircExplicit.name() {
                continue;
            }
            if let Some(rate) = bd_rate(&baseline.points, &result.points) {
                per_codec.insert(result.codec.clone(), rate);
                if result.codec == Anchor::GircInferred.name() {
                    mechanism_bd_rate.insert(name.clone(), rate);
                }
            }
        }
        bd_rate_vs_baseline.insert(name, per_codec);
    }

    // The margin sweep, when asked for, is run as a separate experiment
    // so the main report stays a single configuration.
    let margin_sweep = match &args.margin_sweep {
        None => None,
        Some(margins) => Some(run_margin_sweep(
            &args.sequence,
            &qps,
            &predictor,
            margins,
            args.frames,
        )?),
    };

    let report = Report {
        version: 1,
        generated_utc: utc_now(),
        tool_versions: tool_versions(),
        results,
        bd_rate_vs_baseline,
        mechanism_bd_rate,
        margin_sweep,
    };

    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&args.report, &json)
        .with_context(|| format!("writing the report to {}", args.report.display()))?;

    eprintln!("wrote {}", args.report.display());
    for (sequence, rate) in &report.mechanism_bd_rate {
        eprintln!("mechanism BD-rate on {sequence}: {rate:+.3}%");
    }
    Ok(())
}

/// The current UTC time in RFC 3339 form, without pulling in a date crate.
fn utc_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Days since the epoch, converted with the civil-from-days algorithm.
    let days = (seconds / 86_400) as i64;
    let time_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60
    )
}

/// Convert a day count since the Unix epoch into a civil date.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// The versions of the tools the report was produced with.
fn tool_versions() -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    versions.insert("girc-bench".into(), env!("CARGO_PKG_VERSION").to_string());

    for (name, args) in [
        ("ffmpeg", vec!["-version"]),
        ("vvencapp", vec!["--version"]),
    ] {
        let version = Command::new(name)
            .args(&args)
            .output()
            .ok()
            .and_then(|out| {
                // The first line may end in the middle of a multi-byte
                // character, so the text is decoded before the line is
                // taken.
                let text = String::from_utf8_lossy(&out.stdout);
                text.lines().next().map(|line| line.to_string())
            })
            .unwrap_or_else(|| "unavailable".to_string());
        versions.insert(name.to_string(), version);
    }

    // x265 is reached through ffmpeg's libx265 encoder, so its version
    // comes from ffmpeg rather than from an x265 binary.
    versions.insert(
        "x265".into(),
        Command::new("ffmpeg")
            .args(["-hide_banner", "-h", "encoder=libx265"])
            .output()
            .ok()
            .and_then(|out| {
                let text = String::from_utf8_lossy(&out.stdout);
                text.lines()
                    .find(|line| line.contains("Encoder libx265"))
                    .map(|line| line.trim().to_string())
            })
            .unwrap_or_else(|| "unavailable".to_string()),
    );

    versions
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two identical curves must give a BD-rate of zero.
    #[test]
    fn identical_curves_have_zero_bd_rate() {
        let points: Vec<RatePoint> = (0..4)
            .map(|i| RatePoint {
                qp: 20 + i * 5,
                bits: 1000u64 * (i as u64 + 1),
                psnr_db: 30.0 + i as f64 * 2.0,
            })
            .collect();
        let rate = bd_rate(&points, &points).unwrap();
        assert!(rate.abs() < 1e-6, "identical curves gave {rate}");
    }

    /// A codec that spends ten percent more bits at the same quality must
    /// report a BD-rate of about +10%.
    #[test]
    fn ten_percent_more_bits_gives_ten_percent_bd_rate() {
        let reference: Vec<RatePoint> = (0..4)
            .map(|i| RatePoint {
                qp: 20 + i * 5,
                bits: 1000u64 * (i as u64 + 1),
                psnr_db: 30.0 + i as f64 * 2.0,
            })
            .collect();
        let test: Vec<RatePoint> = reference
            .iter()
            .map(|p| RatePoint {
                qp: p.qp,
                bits: (p.bits as f64 * 1.1) as u64,
                psnr_db: p.psnr_db,
            })
            .collect();
        let rate = bd_rate(&reference, &test).unwrap();
        assert!((rate - 10.0).abs() < 0.5, "expected about +10%, got {rate}");
    }

    /// Ten percent fewer bits must report about -10%.
    #[test]
    fn ten_percent_fewer_bits_gives_minus_ten_percent() {
        let reference: Vec<RatePoint> = (0..4)
            .map(|i| RatePoint {
                qp: 20 + i * 5,
                bits: 1000u64 * (i as u64 + 1),
                psnr_db: 30.0 + i as f64 * 2.0,
            })
            .collect();
        let test: Vec<RatePoint> = reference
            .iter()
            .map(|p| RatePoint {
                qp: p.qp,
                bits: (p.bits as f64 * 0.9) as u64,
                psnr_db: p.psnr_db,
            })
            .collect();
        let rate = bd_rate(&reference, &test).unwrap();
        assert!((rate + 10.0).abs() < 0.5, "expected about -10%, got {rate}");
    }

    /// Two points at the same quality cannot be fitted, because the
    /// polynomial would be singular.
    #[test]
    fn degenerate_curves_are_rejected() {
        let points = vec![
            RatePoint {
                qp: 20,
                bits: 1000,
                psnr_db: 30.0,
            },
            RatePoint {
                qp: 25,
                bits: 2000,
                psnr_db: 30.0,
            },
            RatePoint {
                qp: 30,
                bits: 3000,
                psnr_db: 32.0,
            },
            RatePoint {
                qp: 35,
                bits: 4000,
                psnr_db: 34.0,
            },
        ];
        assert!(bd_rate(&points, &points).is_none());
    }

    /// The points may be given in any order; the fit sorts them.
    #[test]
    fn the_point_order_does_not_matter() {
        let points: Vec<RatePoint> = (0..4)
            .map(|i| RatePoint {
                qp: 20 + i * 5,
                bits: 1000 * (i as u64 + 1),
                psnr_db: 30.0 + i as f64 * 2.0,
            })
            .collect();
        let mut reversed = points.clone();
        reversed.reverse();
        let forward_rate = bd_rate(&points, &points).unwrap();
        let reversed_rate = bd_rate(&reversed, &reversed).unwrap();
        assert!(
            (forward_rate - reversed_rate).abs() < 1e-9,
            "{forward_rate} vs {reversed_rate}"
        );
    }

    /// Identical planes have infinite PSNR; different planes have a finite
    /// value that matches the definition.
    #[test]
    fn psnr_matches_the_definition() {
        assert!(psnr(&[10, 20], &[10, 20]).is_infinite());
        let value = psnr(&[0, 0], &[10, 10]);
        let expected = 10.0 * (255.0f64 * 255.0 / 100.0).log10();
        assert!((value - expected).abs() < 1e-9, "{value} vs {expected}");
    }
}
