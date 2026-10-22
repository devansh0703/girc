use girc_core::codec::*;
use girc_core::intra::IntraMode;
use girc_core::quant::*;
use girc_core::transform::{forward_mode, inverse_mode, TransformMode};
use girc_core::yuv::Yuv4Frame;

fn main() {
    let frame = Yuv4Frame::read(std::path::Path::new("data/seq/akiyo_cif.y4m")).unwrap();
    let picture = frame.luma_picture();
    let config = EncoderConfig::default();
    let bs = 8usize;
    let mut recon = Picture::filled(picture.width, picture.height, 0);
    let mut spreads: Vec<f32> = Vec::new();
    let mut best_gaps: Vec<f32> = Vec::new();
    for by in 0..picture.height / bs {
        for bx in 0..picture.width / bs {
            let reference = reference_samples_for(&recon, bx, by);
            let original = block_samples(&picture, bx, by);
            let mut costs: Vec<f32> = Vec::new();
            for mode in TransformMode::ALL {
                let pred = girc_core::intra::predict(IntraMode::Planar, bs, &reference);
                let residual: Vec<i32> = original
                    .iter()
                    .zip(pred.iter())
                    .map(|(o, p)| *o as i32 - p)
                    .collect();
                let c = forward_mode(mode, bs, &residual).unwrap();
                let params = QuantParams::new(config.qp, bs);
                let levels = quantize_with(&c, &params);
                let dq = dequantize_with(&levels, &params);
                let s = inverse_mode(mode, bs, &dq).unwrap();
                let rec: Vec<i32> = pred.iter().zip(s.iter()).map(|(p, x)| p + x).collect();
                let dist = sse(
                    &original.iter().map(|v| *v as i32).collect::<Vec<_>>(),
                    &rec,
                );
                costs.push(dist as f32);
            }
            let min = costs.iter().cloned().fold(f32::INFINITY, f32::min);
            let max = costs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            spreads.push((max - min) / min.max(1.0));
            let mut sorted = costs.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            best_gaps.push((sorted[1] - sorted[0]) / sorted[0].max(1.0));
            // commit
            let (m, _, _, rb, _) = search_block(&original, &reference, &config);
            let _ = m;
            for y in 0..bs {
                for x in 0..bs {
                    recon.set_sample(bx * bs + x, by * bs + y, rb[y * bs + x].clamp(0, 255) as u8);
                }
            }
        }
    }
    spreads.sort_by(|a, b| a.partial_cmp(b).unwrap());
    best_gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = spreads.len();
    println!("blocks {}", n);
    println!(
        "relative spread (max-min)/min: p10 {:.4} p50 {:.4} p90 {:.4}",
        spreads[n / 10],
        spreads[n / 2],
        spreads[n * 9 / 10]
    );
    println!(
        "gap between best and 2nd best:    p10 {:.4} p50 {:.4} p90 {:.4}",
        best_gaps[n / 10],
        best_gaps[n / 2],
        best_gaps[n * 9 / 10]
    );
}
