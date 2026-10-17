//! Live sources: cameras, opened on a thread of their own (starting a
//! capture takes a while), then read frame by frame.

use std::sync::mpsc::{self, Receiver};

use prism_media::Frame;

#[cfg(target_os = "macos")]
type Device = prism_media::Camera;

/// A device that does not exist on this system.
#[cfg(not(target_os = "macos"))]
struct Device;

#[cfg(not(target_os = "macos"))]
impl Device {
    fn open(_id: &str) -> Result<Self, String> {
        Err("caméras non prises en charge sur ce système".into())
    }
    fn take_frame(&self) -> Option<Frame> {
        None
    }
    fn recycle(&self, _frame: Frame) {}
}

/// The cameras connected now, as identifier and name.
pub fn cameras() -> Vec<(String, String)> {
    #[cfg(target_os = "macos")]
    {
        prism_media::cameras()
            .into_iter()
            .map(|camera| (camera.id, camera.name))
            .collect()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

enum State {
    Opening(Receiver<Result<Device, String>>),
    Ready(Device),
    Failed(String),
}

/// A camera opening or capturing.
pub(crate) struct Live {
    state: State,
}

impl Live {
    /// Starts opening the camera `id`.
    pub(crate) fn open(id: String) -> Self {
        let (sender, receiver) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("prism-camera".into())
            .spawn(move || {
                #[cfg(target_os = "macos")]
                let device = Device::open(&id).map_err(|error| error.to_string());
                #[cfg(not(target_os = "macos"))]
                let device = Device::open(&id);
                let _ = sender.send(device);
            });
        Self {
            state: match spawned {
                Ok(_) => State::Opening(receiver),
                Err(error) => State::Failed(error.to_string()),
            },
        }
    }

    /// Looks whether the camera finished opening.
    pub(crate) fn poll(&mut self) {
        if let State::Opening(receiver) = &self.state {
            match receiver.try_recv() {
                Ok(Ok(device)) => self.state = State::Ready(device),
                Ok(Err(message)) => self.state = State::Failed(message),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.state = State::Failed("ouverture de la caméra interrompue".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }

    /// Why the camera cannot be used.
    pub(crate) fn error(&self) -> Option<&str> {
        match &self.state {
            State::Failed(message) => Some(message),
            _ => None,
        }
    }

    /// The newest frame captured, if a new one came.
    pub(crate) fn take_frame(&self) -> Option<Frame> {
        match &self.state {
            State::Ready(device) => device.take_frame(),
            _ => None,
        }
    }

    /// Gives a frame back to be filled again.
    pub(crate) fn recycle(&self, frame: Frame) {
        if let State::Ready(device) = &self.state {
            device.recycle(frame);
        }
    }
}
