//! YUV4MPEG2 (`.y4m`) reading and writing.
//!
//! GIRC codes the luma plane of a YUV 4:2:0 frame. The Y4M container is
//! used because it is the format the Xiph.Org test sequences ship in, and
//! because it is a trivial, well-documented, lossless container: a text
//! header line followed by raw planar frames.
//!
//! ```text
//! YUV4MPEG2 W352 H288 F25:1 Ip A1:1 C420jpeg XYSCSS=420JPEG
//! FRAME
//! <raw Y plane><raw U plane><raw V plane>
//! FRAME
//! ...
//! ```

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::codec::Picture;

/// A YUV 4:2:0 frame read from a Y4M file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Yuv4Frame {
    pub width: usize,
    pub height: usize,
    /// The frame rate numerator and denominator.
    pub fps_num: u32,
    pub fps_den: u32,
    /// The luma plane.
    pub y: Vec<u8>,
    /// The chroma planes, at half resolution.
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

impl Yuv4Frame {
    /// Read the first frame of a Y4M file.
    pub fn read(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("opening the Y4M file at {}", path.display()))?;
        let mut reader = BufReader::new(file);
        let mut header = Vec::new();
        let mut byte = [0u8; 1];

        // The header ends at the first newline. It is read as bytes,
        // because the header is ASCII and a byte-wise read cannot land in
        // the middle of a character.
        loop {
            if reader.read(&mut byte)? == 0 {
                bail!("the Y4M file at {} ended before its header", path.display());
            }
            header.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
            if header.len() > 1024 {
                bail!("the Y4M header at {} is unreasonably long", path.display());
            }
        }

        // The header is ASCII, but the byte slice it came from may not
        // start on a character boundary, so the text is decoded from the
        // bytes rather than sliced out of the decoded string.
        let mut width = 0usize;
        let mut height = 0usize;
        let mut fps_num = 25u32;
        let mut fps_den = 1u32;
        let mut colour_space = String::from("420jpeg");

        // The Y4M header is a sequence of space-separated tokens, each of
        // which is a one-letter tag followed by its value, for example
        // "W352", "H288", "F25:1" and "C420jpeg".
        //
        // The tokens are taken from the bytes rather than from the decoded
        // string: the header is ASCII, and a byte that is not valid UTF-8
        // would become a replacement character in the decoded string, so
        // slicing the string could split a character.
        for token in header.split(|b| b.is_ascii_whitespace()) {
            if token.is_empty() || token == b"YUV4MPEG2" {
                continue;
            }
            let tag = token[0];
            let value = &token[1..];
            let value_text = String::from_utf8_lossy(value);
            match tag {
                b'W' => width = value_text.parse().context("parsing the Y4M width")?,
                b'H' => height = value_text.parse().context("parsing the Y4M height")?,
                b'F' => {
                    let colon = value
                        .iter()
                        .position(|b| *b == b':')
                        .context("parsing the Y4M frame rate")?;
                    let (n, d) = value.split_at(colon);
                    fps_num = String::from_utf8_lossy(n)
                        .parse()
                        .context("parsing the Y4M frame rate")?;
                    fps_den = String::from_utf8_lossy(&d[1..])
                        .parse()
                        .context("parsing the Y4M frame rate")?;
                }
                b'C' => colour_space = value_text.to_string(),
                _ => {}
            }
        }

        if width == 0 || height == 0 {
            bail!(
                "the Y4M header at {} does not declare a size",
                path.display()
            );
        }
        if !colour_space.starts_with("420") {
            bail!(
                "the Y4M file at {} is {colour_space}; GIRC needs 4:2:0 content",
                path.display()
            );
        }

        // The frame marker.
        let mut marker = Vec::new();
        loop {
            if reader.read(&mut byte)? == 0 {
                bail!(
                    "the Y4M file at {} ended before its first frame",
                    path.display()
                );
            }
            marker.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        if !marker.trim_ascii().starts_with(b"FRAME") {
            bail!(
                "the Y4M file at {} does not start with a FRAME marker",
                path.display()
            );
        }

        let luma_len = width * height;
        let chroma_len = (width / 2) * (height / 2);
        let mut y = vec![0u8; luma_len];
        let mut u = vec![0u8; chroma_len];
        let mut v = vec![0u8; chroma_len];
        reader
            .read_exact(&mut y)
            .with_context(|| format!("reading the luma plane of {}", path.display()))?;
        reader
            .read_exact(&mut u)
            .with_context(|| format!("reading the U plane of {}", path.display()))?;
        reader
            .read_exact(&mut v)
            .with_context(|| format!("reading the V plane of {}", path.display()))?;

        Ok(Self {
            width,
            height,
            fps_num,
            fps_den,
            y,
            u,
            v,
        })
    }

    /// The luma plane as a GIRC picture.
    pub fn luma_picture(&self) -> Picture {
        Picture {
            width: self.width,
            height: self.height,
            samples: self.y.clone(),
        }
    }

    /// Build a frame from a luma picture, filling the chroma planes with a
    /// neutral value.
    pub fn from_luma_picture(picture: &Picture, chroma: u8) -> Self {
        let chroma_len = (picture.width / 2) * (picture.height / 2);
        Self {
            width: picture.width,
            height: picture.height,
            fps_num: 25,
            fps_den: 1,
            y: picture.samples.clone(),
            u: vec![chroma; chroma_len],
            v: vec![chroma; chroma_len],
        }
    }

    /// Write the frame as a single-frame Y4M file.
    pub fn write(&self, path: &Path) -> Result<()> {
        let file = File::create(path)
            .with_context(|| format!("creating the Y4M file at {}", path.display()))?;
        let mut writer = BufWriter::new(file);
        write!(
            writer,
            "YUV4MPEG2 W{} H{} F{}:{} Ip A1:1 C420jpeg\n",
            self.width, self.height, self.fps_num, self.fps_den
        )?;
        writer.write_all(b"FRAME\n")?;
        writer.write_all(&self.y)?;
        writer.write_all(&self.u)?;
        writer.write_all(&self.v)?;
        writer.flush()?;
        Ok(())
    }
}

/// Read every frame of a Y4M file.
///
/// GIRC codes one picture at a time, so a multi-frame file is read frame by
/// frame by the caller; this helper returns the first frame and the number
/// of frames the file holds.
pub fn count_frames(path: &Path) -> Result<usize> {
    let file =
        File::open(path).with_context(|| format!("opening the Y4M file at {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    // The header ends at the first newline; each frame marker is a "FRAME\n".
    let header_end = buffer
        .iter()
        .position(|b| *b == b'\n')
        .context("the Y4M file has no header")?;
    let body = &buffer[header_end + 1..];
    let mut frames = 0usize;
    let mut i = 0usize;
    while i + 5 <= body.len() {
        if &body[i..i + 5] == b"FRAME" {
            frames += 1;
            i += 5;
        } else {
            i += 1;
        }
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_test_y4m(path: &Path, width: usize, height: usize, frames: usize) {
        let mut file = File::create(path).unwrap();
        write!(
            file,
            "YUV4MPEG2 W{width} H{height} F25:1 Ip A1:1 C420jpeg\n"
        )
        .unwrap();
        for _ in 0..frames {
            file.write_all(b"FRAME\n").unwrap();
            file.write_all(&vec![80u8; width * height]).unwrap();
            file.write_all(&vec![128u8; (width / 2) * (height / 2)])
                .unwrap();
            file.write_all(&vec![128u8; (width / 2) * (height / 2)])
                .unwrap();
        }
    }

    #[test]
    fn reads_a_y4m_frame() {
        let dir = std::env::temp_dir().join("girc-yuv-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.y4m");
        write_test_y4m(&path, 16, 16, 2);

        let frame = Yuv4Frame::read(&path).unwrap();
        assert_eq!(frame.width, 16);
        assert_eq!(frame.height, 16);
        assert_eq!(frame.y.len(), 256);
        assert_eq!(frame.u.len(), 64);
        assert_eq!(frame.y[0], 80);
        assert_eq!(count_frames(&path).unwrap(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn round_trips_through_a_file() {
        let dir = std::env::temp_dir().join("girc-yuv-roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.y4m");
        write_test_y4m(&path, 8, 8, 1);

        let frame = Yuv4Frame::read(&path).unwrap();
        let out = dir.join("out.y4m");
        frame.write(&out).unwrap();
        let reread = Yuv4Frame::read(&out).unwrap();
        assert_eq!(frame, reread);

        std::fs::remove_dir_all(&dir).ok();
    }
}
