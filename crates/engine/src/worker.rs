//! Decoding thread of one media file.
//!
//! The render thread tells the worker which frames it needs, in order of
//! urgency; the worker decodes them and sends them back. A new request
//! replaces the previous one, so the worker never falls behind on frames
//! that are no longer useful. Frame buffers travel back to the worker once
//! used, so decoding allocates nothing in the steady state.

use std::collections::VecDeque;
use std::error::Error;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use prism_media::{HapDecoder, HapFrame, VideoTrack};

/// Message from the render thread to a worker.
pub(crate) enum Request {
    /// Frames to decode, most urgent first. Replaces the previous list.
    Want(Vec<usize>),
    /// A frame buffer the render thread no longer needs.
    Recycle(HapFrame),
}

/// Message from a worker to the render thread.
pub(crate) enum Event {
    /// The file is open.
    Opened(VideoTrack),
    /// The file could not be opened; the worker stops.
    OpenFailed(String),
    /// A decoded frame.
    Frame(usize, HapFrame),
    /// A frame could not be decoded.
    FrameFailed(usize, String),
}

/// The render thread's end of a worker.
pub(crate) struct Worker {
    requests: Sender<Request>,
    events: Receiver<Event>,
}

impl Worker {
    /// Starts a worker decoding `path`. The thread ends when the returned
    /// value is dropped.
    pub(crate) fn spawn(path: PathBuf) -> std::io::Result<Self> {
        let (requests, request_receiver) = mpsc::channel();
        let (event_sender, events) = mpsc::channel();
        thread::Builder::new()
            .name("prism-decode".into())
            .spawn(move || run(path, request_receiver, event_sender))?;
        Ok(Self { requests, events })
    }

    /// Asks for `frames`, most urgent first.
    pub(crate) fn want(&self, frames: Vec<usize>) {
        // A send error means the worker stopped after reporting why.
        let _ = self.requests.send(Request::Want(frames));
    }

    /// Gives a frame buffer back for reuse.
    pub(crate) fn recycle(&self, frame: HapFrame) {
        let _ = self.requests.send(Request::Recycle(frame));
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

fn run(path: PathBuf, requests: Receiver<Request>, events: Sender<Event>) {
    let mut decoder = match HapDecoder::open(&path) {
        Ok(decoder) => decoder,
        Err(error) => {
            let _ = events.send(Event::OpenFailed(error_chain(&error)));
            return;
        }
    };
    if events.send(Event::Opened(decoder.track().clone())).is_err() {
        return;
    }

    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut spare: Vec<HapFrame> = Vec::new();
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
                Request::Want(frames) => queue = frames.into(),
                Request::Recycle(frame) => spare.push(frame),
            }
            continue;
        }

        let Some(index) = queue.pop_front() else {
            continue;
        };
        let mut frame = spare.pop().unwrap_or_default();
        let event = match decoder.decode_frame(index, &mut frame) {
            Ok(()) => Event::Frame(index, frame),
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
