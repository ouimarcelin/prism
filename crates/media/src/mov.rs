//! Minimal reader for QuickTime (`.mov`) and MP4 files.
//!
//! It finds the first video track and builds its sample table, so that any
//! frame can be read directly from the file without decoding its neighbours.
//! Edit lists are ignored: frames are played as stored.

use std::io::{self, Read, Seek, SeekFrom};

/// Four-character code of a codec or atom.
pub type FourCc = [u8; 4];

/// Largest `moov` atom read into memory. Real files stay far below.
const MAX_MOVIE_HEADER: u64 = 256 * 1024 * 1024;

/// Largest frame count accepted: 24 hours at 240 frames per second.
const MAX_FRAMES: usize = 24 * 3600 * 240;

/// Error raised while reading the container.
#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    /// The file could not be read.
    #[error("cannot read the file")]
    Io(#[from] io::Error),
    /// The file structure is invalid or truncated.
    #[error("invalid QuickTime/MP4 file: {0}")]
    Malformed(&'static str),
    /// The file has no video track.
    #[error("the file has no video track")]
    NoVideoTrack,
}

/// Location and timing of one frame in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Byte offset of the frame in the file.
    pub offset: u64,
    /// Byte size of the frame.
    pub size: u32,
    /// Start time, in track timescale units.
    pub time: u64,
    /// Duration, in track timescale units.
    pub duration: u32,
}

/// The video track of a file and its sample table.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoTrack {
    /// Codec of the frames, for example `Hap1`.
    pub codec: FourCc,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Time units per second.
    pub timescale: u32,
    samples: Vec<Sample>,
}

impl VideoTrack {
    /// Reads the first video track of a file.
    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, ContainerError> {
        let movie = read_movie_atom(reader)?;
        for track in children(&movie, *b"trak") {
            let track = track?;
            if let Some(video) = parse_track(track)? {
                return Ok(video);
            }
        }
        Err(ContainerError::NoVideoTrack)
    }

    /// Codec as text, for messages.
    pub fn codec_name(&self) -> String {
        String::from_utf8_lossy(&self.codec).into_owned()
    }

    /// Number of frames.
    pub fn frame_count(&self) -> usize {
        self.samples.len()
    }

    /// Location and timing of a frame.
    pub fn sample(&self, index: usize) -> Option<Sample> {
        self.samples.get(index).copied()
    }

    /// Total duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        let end = self
            .samples
            .last()
            .map_or(0, |s| s.time + u64::from(s.duration));
        end as f64 / f64::from(self.timescale)
    }

    /// Average frames per second.
    pub fn frame_rate(&self) -> f64 {
        let duration = self.duration_seconds();
        if duration > 0.0 {
            self.samples.len() as f64 / duration
        } else {
            0.0
        }
    }

    /// Index of the frame shown at `seconds`, clamped to the track.
    pub fn frame_at(&self, seconds: f64) -> usize {
        let target = (seconds.max(0.0) * f64::from(self.timescale)) as u64;
        self.samples
            .partition_point(|sample| sample.time <= target)
            .saturating_sub(1)
    }

    /// Reads the raw bytes of a frame into `buffer`.
    pub fn read_sample<R: Read + Seek>(
        &self,
        reader: &mut R,
        index: usize,
        buffer: &mut Vec<u8>,
    ) -> Result<(), ContainerError> {
        let sample = self
            .sample(index)
            .ok_or(ContainerError::Malformed("frame index out of range"))?;
        reader.seek(SeekFrom::Start(sample.offset))?;
        buffer.resize(sample.size as usize, 0);
        reader.read_exact(buffer)?;
        Ok(())
    }
}

/// Reads the `moov` atom, skipping every other top-level atom.
fn read_movie_atom<R: Read + Seek>(reader: &mut R) -> Result<Vec<u8>, ContainerError> {
    reader.seek(SeekFrom::Start(0))?;
    loop {
        let mut header = [0u8; 8];
        match reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(ContainerError::Malformed("no moov atom"));
            }
            Err(error) => return Err(error.into()),
        }
        let kind: FourCc = [header[4], header[5], header[6], header[7]];
        let (body_size, to_end) =
            match u32::from_be_bytes([header[0], header[1], header[2], header[3]]) {
                0 => (0, true),
                1 => {
                    let mut large = [0u8; 8];
                    reader.read_exact(&mut large)?;
                    let size = u64::from_be_bytes(large);
                    let body = size
                        .checked_sub(16)
                        .ok_or(ContainerError::Malformed("atom size too small"))?;
                    (body, false)
                }
                size => {
                    let body = u64::from(size)
                        .checked_sub(8)
                        .ok_or(ContainerError::Malformed("atom size too small"))?;
                    (body, false)
                }
            };

        if &kind == b"moov" {
            let mut movie = Vec::new();
            if to_end {
                reader.take(MAX_MOVIE_HEADER).read_to_end(&mut movie)?;
            } else {
                if body_size > MAX_MOVIE_HEADER {
                    return Err(ContainerError::Malformed("moov atom too large"));
                }
                movie.resize(body_size as usize, 0);
                reader.read_exact(&mut movie)?;
            }
            return Ok(movie);
        }
        if to_end {
            return Err(ContainerError::Malformed("no moov atom"));
        }
        let skip =
            i64::try_from(body_size).map_err(|_| ContainerError::Malformed("atom too large"))?;
        reader.seek(SeekFrom::Current(skip))?;
    }
}

/// Atoms contained in a byte slice.
struct Atoms<'a> {
    data: &'a [u8],
}

impl<'a> Iterator for Atoms<'a> {
    type Item = Result<(FourCc, &'a [u8]), ContainerError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() {
            return None;
        }
        let result = split_atom(self.data);
        match result {
            Ok((kind, body, rest)) => {
                self.data = rest;
                Some(Ok((kind, body)))
            }
            Err(error) => {
                self.data = &[];
                Some(Err(error))
            }
        }
    }
}

type SplitAtom<'a> = (FourCc, &'a [u8], &'a [u8]);

fn split_atom(data: &[u8]) -> Result<SplitAtom<'_>, ContainerError> {
    let truncated = ContainerError::Malformed("truncated atom");
    if data.len() < 8 {
        return Err(truncated);
    }
    let kind = [data[4], data[5], data[6], data[7]];
    let (size, header) = match be_u32(data, 0)? {
        0 => (data.len(), 8),
        1 => {
            let size = usize::try_from(be_u64(data, 8)?)
                .map_err(|_| ContainerError::Malformed("atom too large"))?;
            (size, 16)
        }
        size => (size as usize, 8),
    };
    if size < header || size > data.len() {
        return Err(truncated);
    }
    Ok((kind, &data[header..size], &data[size..]))
}

fn atoms(data: &[u8]) -> Atoms<'_> {
    Atoms { data }
}

/// Children of an atom with the given type.
fn children(data: &[u8], wanted: FourCc) -> impl Iterator<Item = Result<&[u8], ContainerError>> {
    atoms(data).filter_map(move |atom| match atom {
        Ok((kind, body)) if kind == wanted => Some(Ok(body)),
        Ok(_) => None,
        Err(error) => Some(Err(error)),
    })
}

/// First child of an atom with the given type.
fn child(data: &[u8], wanted: FourCc) -> Result<Option<&[u8]>, ContainerError> {
    children(data, wanted).next().transpose()
}

fn required<'a>(
    data: &'a [u8],
    wanted: FourCc,
    missing: &'static str,
) -> Result<&'a [u8], ContainerError> {
    child(data, wanted)?.ok_or(ContainerError::Malformed(missing))
}

fn be_u16(data: &[u8], at: usize) -> Result<u16, ContainerError> {
    data.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or(ContainerError::Malformed("truncated table"))
}

fn be_u32(data: &[u8], at: usize) -> Result<u32, ContainerError> {
    data.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(ContainerError::Malformed("truncated table"))
}

fn be_u64(data: &[u8], at: usize) -> Result<u64, ContainerError> {
    data.get(at..at + 8)
        .map(|b| u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
        .ok_or(ContainerError::Malformed("truncated table"))
}

/// Entry count of a full-atom table, checked against the room left for
/// entries of `entry_size` bytes starting at `first`.
fn entry_count(
    data: &[u8],
    count_at: usize,
    first: usize,
    entry_size: usize,
) -> Result<usize, ContainerError> {
    let count = be_u32(data, count_at)? as usize;
    let room = data.len().saturating_sub(first) / entry_size;
    if count > room {
        return Err(ContainerError::Malformed("table count exceeds its atom"));
    }
    Ok(count)
}

/// Builds the video track description from a `trak` atom, or `None` when
/// the track is not video.
fn parse_track(track: &[u8]) -> Result<Option<VideoTrack>, ContainerError> {
    let Some(media) = child(track, *b"mdia")? else {
        return Ok(None);
    };
    let handler = required(media, *b"hdlr", "missing hdlr atom")?;
    if handler.get(8..12) != Some(b"vide".as_slice()) {
        return Ok(None);
    }

    let header = required(media, *b"mdhd", "missing mdhd atom")?;
    let timescale = match header.first() {
        Some(1) => be_u32(header, 20)?,
        Some(_) => be_u32(header, 12)?,
        None => return Err(ContainerError::Malformed("empty mdhd atom")),
    };
    if timescale == 0 {
        return Err(ContainerError::Malformed("zero timescale"));
    }

    let info = required(media, *b"minf", "missing minf atom")?;
    let table = required(info, *b"stbl", "missing stbl atom")?;

    // Sample description: codec and frame size of the first entry.
    let description = required(table, *b"stsd", "missing stsd atom")?;
    let entry = description
        .get(8..)
        .ok_or(ContainerError::Malformed("empty stsd atom"))?;
    if entry.len() < 36 {
        return Err(ContainerError::Malformed(
            "truncated video sample description",
        ));
    }
    let codec = [entry[4], entry[5], entry[6], entry[7]];
    let width = u32::from(be_u16(entry, 32)?);
    let height = u32::from(be_u16(entry, 34)?);

    let samples = sample_table(table)?;
    Ok(Some(VideoTrack {
        codec,
        width,
        height,
        timescale,
        samples,
    }))
}

fn sample_table(table: &[u8]) -> Result<Vec<Sample>, ContainerError> {
    // Sizes.
    let sizes_atom = required(table, *b"stsz", "missing stsz atom")?;
    let uniform_size = be_u32(sizes_atom, 4)?;
    let sizes: Vec<u32> = if uniform_size != 0 {
        // A uniform table stores the count only, with no entry to bound it.
        let count = be_u32(sizes_atom, 8)? as usize;
        if count > MAX_FRAMES {
            return Err(ContainerError::Malformed("too many frames"));
        }
        vec![uniform_size; count]
    } else {
        let count = entry_count(sizes_atom, 8, 12, 4)?;
        (0..count)
            .map(|i| be_u32(sizes_atom, 12 + 4 * i))
            .collect::<Result<_, _>>()?
    };
    let count = sizes.len();

    // Times.
    let times_atom = required(table, *b"stts", "missing stts atom")?;
    let mut times = Vec::with_capacity(count);
    let mut durations = Vec::with_capacity(count);
    let mut now = 0u64;
    for i in 0..entry_count(times_atom, 4, 8, 8)? {
        let repeat = be_u32(times_atom, 8 + 8 * i)?;
        let delta = be_u32(times_atom, 12 + 8 * i)?;
        for _ in 0..repeat {
            if times.len() == count {
                break;
            }
            times.push(now);
            durations.push(delta);
            now += u64::from(delta);
        }
    }
    if times.len() < count {
        return Err(ContainerError::Malformed(
            "time table shorter than sample table",
        ));
    }

    // Chunk offsets.
    let offsets: Vec<u64> = if let Some(offsets) = child(table, *b"stco")? {
        (0..entry_count(offsets, 4, 8, 4)?)
            .map(|i| be_u32(offsets, 8 + 4 * i).map(u64::from))
            .collect::<Result<_, _>>()?
    } else if let Some(offsets) = child(table, *b"co64")? {
        (0..entry_count(offsets, 4, 8, 8)?)
            .map(|i| be_u64(offsets, 8 + 8 * i))
            .collect::<Result<_, _>>()?
    } else {
        return Err(ContainerError::Malformed("missing stco or co64 atom"));
    };

    // Samples per chunk: entries (first chunk, samples per chunk), first chunk
    // numbered from 1 and increasing.
    let chunking_atom = required(table, *b"stsc", "missing stsc atom")?;
    let chunking: Vec<(u32, u32)> = (0..entry_count(chunking_atom, 4, 8, 12)?)
        .map(|i| {
            Ok((
                be_u32(chunking_atom, 8 + 12 * i)?,
                be_u32(chunking_atom, 12 + 12 * i)?,
            ))
        })
        .collect::<Result<_, ContainerError>>()?;
    if chunking.is_empty() && count > 0 {
        return Err(ContainerError::Malformed("empty stsc atom"));
    }

    let mut samples = Vec::with_capacity(count);
    let mut rule = 0;
    for (chunk_index, &chunk_offset) in offsets.iter().enumerate() {
        let chunk_number = chunk_index as u64 + 1;
        while rule + 1 < chunking.len() && u64::from(chunking[rule + 1].0) <= chunk_number {
            rule += 1;
        }
        let per_chunk = chunking.get(rule).map_or(0, |&(_, n)| n);
        let mut offset = chunk_offset;
        for _ in 0..per_chunk {
            let index = samples.len();
            if index == count {
                break;
            }
            samples.push(Sample {
                offset,
                size: sizes[index],
                time: times[index],
                duration: durations[index],
            });
            offset += u64::from(sizes[index]);
        }
    }
    if samples.len() != count {
        return Err(ContainerError::Malformed(
            "chunk table does not cover every sample",
        ));
    }
    Ok(samples)
}
