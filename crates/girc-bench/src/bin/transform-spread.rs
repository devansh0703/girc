//! Measure how much the transform choice actually matters.
//!
//! The mechanism's whole premise is that the rate-distortion search picks
//! a transform that is meaningfully better than the alternatives. This
//! tool measures the spread: for every block, the distortion of the best
//! mode and of the runner-up, and the difference between them as a
//! fraction.
//!
//! A small spread means the choice is a near-tie, and no predictor can
//! predict a near-tie reliably. That is the finding the per-axis
//! predictor is built on, so it is worth measuring rather than
//! asserting.
//!
//! Usage: transform-spread <sequence.y4m> [output.json]

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use girc_core::codec::{
    block_samples, reference_samples_for, search_block, EncoderConfig, Picture, BLOCK_SIZE,
};
use girc_core::intra::{predict, IntraMode};
use girc_core::quant::{dequantize_with, quantize_with, sse, QuantParams};
use girc_core::transform::{forward_mode, inverse_mode, TransformMode};
use girc_core::yuv::Yuv4Frame;

/// The percentile of an ordered slice.
fn percentile(ordered: &[f64], fraction: f64) -> f64 {
    if ordered.is_empty() {
        return 0.0;
    }
    let index = ((ordered.len() as f64 * fraction) as usize).min(ordered.len() - 1);
    ordered[index]
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let sequence = PathBuf::from(
        args.next()
            .context("usage: transform-spread <sequence.y4m> [output.json]")?,
    );
    let output = args.next().map(PathBuf::from);

    let frame = Yuv4Frame::read(&sequence)
        .with_context(|| format!("reading the sequence at {}", sequence.display()))?;
    let picture = frame.luma_picture();
    let config = EncoderConfig::default();
    let mut reconstruction = Picture::filled(picture.width, picture.height, 0);

    let mut spreads: Vec<f64> = Vec::new();
    let mut gaps: Vec<f64> = Vec::new();

    for by in 0..picture.height / BLOCK_SIZE {
        for bx in 0..picture.width / BLOCK_SIZE {
            let reference = reference_samples_for(&reconstruction, bx, by);
            let original = block_samples(&picture, bx, by);
            let prediction = predict(IntraMode::Planar, BLOCK_SIZE, &reference);

            let mut costs: Vec<f64> = Vec::new();
            for mode in TransformMode::ALL {
                let residual: Vec<i32> = original
                    .iter()
                    .zip(prediction.iter())
                    .map(|(o, p)| *o as i32 - p)
                    .collect();
                let coefficients = forward_mode(mode, BLOCK_SIZE, &residual)
                    .map_err(|e| anyhow::anyhow!("the forward transform failed: {e}"))?;
                let params = QuantParams::new(config.qp, BLOCK_SIZE);
                let levels = quantize_with(&coefficients, &params);
                let dequantized = dequantize_with(&levels, &params);
                let samples = inverse_mode(mode, BLOCK_SIZE, &dequantized)
                    .map_err(|e| anyhow::anyhow!("the inverse transform failed: {e}"))?;
                let restored: Vec<i32> = prediction
                    .iter()
                    .zip(samples.iter())
                    .map(|(p, s)| p + s)
                    .collect();
                let distortion = sse(
                    &original.iter().map(|v| *v as i32).collect::<Vec<_>>(),
                    &restored,
                );
                costs.push(distortion as f64);
            }

            let lowest = costs.iter().copied().fold(f64::INFINITY, f64::min);
            let highest = costs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            spreads.push((highest - lowest) / lowest.max(1.0));
            let mut ordered = costs.clone();
            ordered.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            gaps.push((ordered[1] - ordered[0]) / ordered[0].max(1.0));

            // Commit the reconstruction the encoder would have made, so
            // the next block's reference is the real one.
            let (_, _, _, block, _) = search_block(&original, &reference, &config);
            for y in 0..BLOCK_SIZE {
                for x in 0..BLOCK_SIZE {
                    let value = block[y * BLOCK_SIZE + x].clamp(0, 255) as u8;
                    reconstruction.set_sample(bx * BLOCK_SIZE + x, by * BLOCK_SIZE + y, value);
                }
            }
        }
    }

    spreads.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut sorted_gaps = gaps.clone();
    sorted_gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len().max(1) as f64;

    let report = serde_json::json!({
        "sequence": sequence.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
        "blocks": spreads.len(),
        "qp": config.qp,
        "relative_spread": {
            "p10": percentile(&spreads, 0.10),
            "p50": percentile(&spreads, 0.50),
            "p90": percentile(&spreads, 0.90),
            "mean": mean(&spreads),
        },
        "gap_between_best_and_runner_up": {
            "p10": percentile(&sorted_gaps, 0.10),
            "p50": percentile(&sorted_gaps, 0.50),
            "p90": percentile(&sorted_gaps, 0.90),
            "mean": mean(&sorted_gaps),
        },
    });
    let text = serde_json::to_string_pretty(&report)? + "\n";
    println!("{text}");

    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, text)
            .with_context(|| format!("writing the report to {}", path.display()))?;
    }
    Ok(())
}

/// The path of the sequence, for the report.
#[allow(dead_code)]
fn sequence_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}
