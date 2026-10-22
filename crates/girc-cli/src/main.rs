//! Command-line front end for the GIRC reference codec.
//!
//! ```text
//! girc encode --input <y4m> --output <bitstream> [--signalling explicit|inferred]
//!            [--qp N] [--model <model.json>]
//! girc decode --input <bitstream> --output <y4m> [--signalling ...] [--model ...]
//! girc inspect --input <bitstream>
//! ```

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use girc_core::bitstream::BitstreamHeader;
use girc_core::codec::{collect_training_examples, decode, encode, EncoderConfig};
use girc_core::geometry::{TransformPredictor, TransformSignalling};
use girc_core::yuv::Yuv4Frame;

#[derive(Parser, Debug)]
#[command(
    name = "girc",
    about = "GIRC: the Geometry-Inference Reference Codec",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Encode a Y4M file into a GIRC bitstream.
    Encode {
        /// The input Y4M file.
        #[arg(long)]
        input: PathBuf,
        /// The output bitstream.
        #[arg(long)]
        output: PathBuf,
        /// How the transform type is signalled.
        #[arg(long, value_enum, default_value_t = Signalling::Explicit)]
        signalling: Signalling,
        /// The quantization parameter.
        #[arg(long, default_value_t = 27)]
        qp: i32,
        /// A trained predictor model, required for inferred signalling.
        #[arg(long)]
        model: Option<PathBuf>,
    },
    /// Decode a GIRC bitstream into a Y4M file.
    Decode {
        /// The input bitstream.
        #[arg(long)]
        input: PathBuf,
        /// The output Y4M file.
        #[arg(long)]
        output: PathBuf,
        /// How the transform type is signalled; must match the encoder.
        #[arg(long, value_enum, default_value_t = Signalling::Explicit)]
        signalling: Signalling,
        /// A trained predictor model; must match the encoder's.
        #[arg(long)]
        model: Option<PathBuf>,
    },
    /// Print a summary of a GIRC bitstream.
    Inspect {
        /// The input bitstream.
        #[arg(long)]
        input: PathBuf,
    },
    /// Print one JSON record per coded block, for the training pipeline.
    ///
    /// Each record holds the block's geometry features and the transform the
    /// rate-distortion search chose.
    Collect {
        /// The input Y4M file.
        #[arg(long)]
        input: PathBuf,
        /// The quantization parameter.
        #[arg(long, default_value_t = 27)]
        qp: i32,
    },
    /// Report how often the decoder's prediction differs from the encoder's
    /// choice, for the trained model.
    Evaluate {
        /// The input Y4M file.
        #[arg(long)]
        input: PathBuf,
        /// The quantization parameter.
        #[arg(long, default_value_t = 27)]
        qp: i32,
        /// The predictor model to evaluate.
        #[arg(long)]
        model: PathBuf,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Signalling {
    /// Signal the transform type for every block.
    Explicit,
    /// Let the decoder infer the transform type, with a one-bit correction.
    Inferred,
}

impl From<Signalling> for TransformSignalling {
    fn from(value: Signalling) -> Self {
        match value {
            Signalling::Explicit => TransformSignalling::Explicit,
            Signalling::Inferred => TransformSignalling::Inferred,
        }
    }
}

/// Build the encoder configuration from the command-line arguments.
fn build_config(signalling: Signalling, qp: i32, model: Option<&PathBuf>) -> Result<EncoderConfig> {
    let predictor = match model {
        Some(path) => {
            let text = fs::read_to_string(path)
                .with_context(|| format!("reading the predictor model at {}", path.display()))?;
            TransformPredictor::from_json(&text)
                .map_err(|e| anyhow!("loading the predictor model: {e}"))?
        }
        None => TransformPredictor::constant_dct2(),
    };
    Ok(EncoderConfig {
        qp,
        signalling: signalling.into(),
        predictor,
        ..Default::default()
    })
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Encode {
            input,
            output,
            signalling,
            qp,
            model,
        } => {
            let frame = Yuv4Frame::read(&input)
                .with_context(|| format!("reading the input frame at {}", input.display()))?;
            let config = build_config(signalling, qp, model.as_ref())?;
            let picture = frame.luma_picture();
            let result = encode(&picture, &config)?;
            fs::write(&output, &result.bitstream)
                .with_context(|| format!("writing the bitstream to {}", output.display()))?;

            eprintln!(
                "encoded {}x{} qp={} signalling={} blocks={} payload={} bytes mismatches={} signalling_bits={}",
                picture.width,
                picture.height,
                qp,
                signalling_name(signalling),
                result.block_count,
                result.payload_bytes,
                result.prediction_mismatches,
                result.transform_signalling_bins,
            );
        }
        Command::Decode {
            input,
            output,
            signalling,
            model,
        } => {
            let bitstream = fs::read(&input)
                .with_context(|| format!("reading the bitstream at {}", input.display()))?;
            let (header, _) = BitstreamHeader::read(&bitstream)?;
            let config = build_config(signalling, header.qp as i32, model.as_ref())?;
            let picture = decode(&bitstream, &config)?;
            let frame = Yuv4Frame::from_luma_picture(&picture, 128);
            frame
                .write(&output)
                .with_context(|| format!("writing the frame to {}", output.display()))?;
            eprintln!(
                "decoded {}x{} qp={} signalling={}",
                picture.width,
                picture.height,
                header.qp,
                signalling_name(signalling),
            );
        }
        Command::Collect { input, qp } => {
            let frame = Yuv4Frame::read(&input)
                .with_context(|| format!("reading the input frame at {}", input.display()))?;
            let config = EncoderConfig {
                qp,
                ..Default::default()
            };
            let examples = collect_training_examples(&frame.luma_picture(), &config)?;
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            for example in examples {
                let payload = serde_json::json!({
                    "features": example.features.to_vec(),
                    "transform": example.transform.as_index(),
                });
                writeln!(out, "{payload}").context("writing the collection records")?;
            }
        }
        Command::Evaluate { input, qp, model } => {
            let frame = Yuv4Frame::read(&input)
                .with_context(|| format!("reading the input frame at {}", input.display()))?;
            let text = fs::read_to_string(&model)
                .with_context(|| format!("reading the model at {}", model.display()))?;
            let predictor = TransformPredictor::from_json(&text)
                .map_err(|e| anyhow!("loading the predictor model: {e}"))?;
            let config = EncoderConfig {
                qp,
                signalling: TransformSignalling::Inferred,
                predictor,
                ..Default::default()
            };
            let result = encode(&frame.luma_picture(), &config)?;
            let payload = serde_json::json!({
                "sequence": input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                "qp": qp,
                "block_count": result.block_count,
                "prediction_mismatches": result.prediction_mismatches,
                "mismatch_rate": result.prediction_mismatches as f64 / result.block_count as f64,
                "transform_signalling_bins": result.transform_signalling_bins,
            });
            println!("{payload}");
        }
        Command::Inspect { input } => {
            let bitstream = fs::read(&input)
                .with_context(|| format!("reading the bitstream at {}", input.display()))?;
            let (header, total) = BitstreamHeader::read(&bitstream)?;
            println!("magic:      GIRC");
            println!("version:    {}", header.version);
            println!("size:       {}x{}", header.width, header.height);
            println!("qp:         {}", header.qp);
            println!("total:      {total} bytes");
            println!("payload:    {} bytes", total - BitstreamHeader::SIZE);
        }
    }
    Ok(())
}

/// The display name of a signalling mode.
fn signalling_name(signalling: Signalling) -> &'static str {
    match signalling {
        Signalling::Explicit => "explicit",
        Signalling::Inferred => "inferred",
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
