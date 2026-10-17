//! Decoding thread of one media file.
//!
//! The render thread tells the worker which frames it needs, in order of
//! urgency; the worker decodes them and sends them back. A new request
//! replaces the previous one, so the worker never falls behind on frames
//! that are no longer useful. Frame buffers travel back to the worker once
//! used, so decoding allocates nothing in the steady state.
//!
//! The worker never decodes a frame the render thread still holds or has not
//! received yet: requests may list them again, since the render thread
//! cannot see frames in transit. Movie decoders that read forward (H.264)
//! rely on it: decoding a frame twice would send them back to a key frame.

use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use prism_core::MediaSource;
use prism_media::{Frame, FrameSource, MediaInfo};

/// Message from the render thread to a worker.
pub(crate) enum Request {
    /// Frames to decode, most urgent first. Replaces the previous list.
    Want(Vec<usize>),
    /// A frame buffer the render thread no longer needs, with the index of
    /// the frame it held.
    Recycle(usize, Frame),
}

/// Message from a worker to the render thread.
pub(crate) enum Event {
    /// The media is open.
    Opened(MediaInfo),
    /// The file could not be opened; the worker stops.
    OpenFailed(String),
    /// A decoded frame.
    Frame(usize, Frame),
    /// A frame could not be decoded.
    FrameFailed(usize, String),
}

/// The render thread's end of a worker.
pub(crate) struct Worker {
    requests: Sender<Request>,
    events: Receiver<Event>,
}

impl Worker {
    /// Starts a worker decoding `source`. The thread ends when the returned
    /// value is dropped.
    pub(crate) fn spawn(source: MediaSource) -> std::io::Result<Self> {
        let (requests, request_receiver) = mpsc::channel();
        let (event_sender, events) = mpsc::channel();
        thread::Builder::new()
            .name("prism-decode".into())
            .spawn(move || run(&source, request_receiver, event_sender))?;
        Ok(Self { requests, events })
    }

    /// Asks for `frames`, most urgent first.
    pub(crate) fn want(&self, frames: Vec<usize>) {
        // A send error means the worker stopped after reporting why.
        let _ = self.requests.send(Request::Want(frames));
    }

    /// Gives back the buffer of frame `index`, which may then be decoded
    /// again if asked.
    pub(crate) fn recycle(&self, index: usize, frame: Frame) {
        let _ = self.requests.send(Request::Recycle(index, frame));
    }

    /// Next event, if any, without waiting.
    pub(crate) fn try_event(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }
}

/// Message of an error and of all its causes.
pub(crate) fn error_chain(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

/// Opens the media of a clip.
fn open(source: &MediaSource) -> Result<Box<dyn FrameSource>, prism_media::DecodeError> {
    match source {
        MediaSource::File { path } => prism_media::open_file(path),
        MediaSource::ImageSequence { path, frame_rate } => {
            prism_media::open_sequence(path, *frame_rate)
        }
        // Drawn by the render thread: nothing to decode.
        MediaSource::Generator { generator } => Err(prism_media::DecodeError::UnsupportedCodec(
            format!("générateur {}", generator.kind),
        )),
        // Read live by the render thread.
        MediaSource::Camera { name, .. } | MediaSource::Syphon { name, .. } => Err(
            prism_media::DecodeError::UnsupportedCodec(format!("source en direct {name}")),
        ),
    }
}

fn run(source: &MediaSource, requests: Receiver<Request>, events: Sender<Event>) {
    crate::priority::decode_thread();
    let mut decoder = match open(source) {
        Ok(decoder) => decoder,
        Err(error) => {
            let _ = events.send(Event::OpenFailed(error_chain(&error)));
            return;
        }
    };
    if events.send(Event::Opened(decoder.info().clone())).is_err() {
        return;
    }

    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut spare: Vec<Frame> = Vec::new();
    // Frames sent and not yet given back.
    let mut delivered: HashSet<usize> = HashSet::new();
    loop {
        // Wait when idle; otherwise take pending messages without waiting.
        let message = if queue.is_empty() {
            match requests.recv() {
                Ok(message) => Some(message),
                Err(_) => return,
            }
        } else {
            match requests.try_recv() {
                Ok(message) => Some(message),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        };
        if let Some(message) = message {
            match message {
                Request::Want(frames) => {
                    queue = frames
                        .into_iter()
                        .filter(|index| !delivered.contains(index))
                        .collect();
                }
                Request::Recycle(index, frame) => {
                    delivered.remove(&index);
                    spare.push(frame);
                }
            }
            continue;
        }

        let Some(index) = queue.pop_front() else {
            continue;
        };
        let mut frame = spare.pop().unwrap_or_default();
        let event = match decoder.decode(index, &mut frame) {
            Ok(()) => {
                delivered.insert(index);
                Event::Frame(index, frame)
            }
            Err(error) => {
                spare.push(frame);
                Event::FrameFailed(index, error_chain(&error))
            }
        };
        if events.send(event).is_err() {
            return;
        }
    }
}
