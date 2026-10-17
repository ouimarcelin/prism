//! Cameras: USB and built-in cameras, and capture devices that show up as
//! cameras, read live through AVFoundation.
//!
//! A capture session runs on a dispatch queue of its own; each image it
//! delivers is copied into a frame, which replaces the frame not taken yet.
//! The reader takes the newest frame when it wants one, and gives the old
//! one back to be filled again.

// Calls into Apple frameworks. Every block states why it is sound.
#![allow(unsafe_code)]

use std::sync::{Arc, Mutex, PoisonError};

use dispatch2::DispatchQueue;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_av_foundation::{
    AVAuthorizationStatus, AVCaptureConnection, AVCaptureDevice, AVCaptureDeviceInput,
    AVCaptureOutput, AVCaptureSession, AVCaptureVideoDataOutput,
    AVCaptureVideoDataOutputSampleBufferDelegate, AVMediaTypeVideo,
};
use objc2_core_foundation::CFString;
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::{
    kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString};

use crate::avf::copy_planes;
use crate::decoder::DecodeError;
use crate::source::Frame;

/// A camera of the computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraDevice {
    /// Identifier, stable across reconnections and restarts.
    pub id: String,
    /// Name shown to the user.
    pub name: String,
}

/// The cameras connected now.
pub fn cameras() -> Vec<CameraDevice> {
    autoreleasepool(|_| {
        // SAFETY: reading an immutable framework constant.
        let Some(video) = (unsafe { AVMediaTypeVideo }) else {
            return Vec::new();
        };
        // SAFETY: class method listing the devices; the simpler call is
        // enough to list them.
        #[allow(deprecated)]
        let devices = unsafe { AVCaptureDevice::devicesWithMediaType(video) };
        devices
            .iter()
            .map(|device| CameraDevice {
                // SAFETY: property getters of a valid device.
                id: unsafe { device.uniqueID() }.to_string(),
                name: unsafe { device.localizedName() }.to_string(),
            })
            .collect()
    })
}

/// Frames delivered and not taken yet, and frames to fill again.
#[derive(Default)]
struct Shared {
    latest: Mutex<Option<Frame>>,
    spare: Mutex<Option<Frame>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; the class only
    // adds the delegate method below.
    #[unsafe(super(NSObject))]
    #[name = "PrismCameraDelegate"]
    #[ivars = Arc<Shared>]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for Delegate {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        fn did_output(
            &self,
            _output: &AVCaptureOutput,
            sample: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            let shared = self.ivars();
            // SAFETY: getter of the valid sample buffer of this call.
            let Some(buffer) = (unsafe { sample.image_buffer() }) else {
                return;
            };
            let mut frame = shared
                .spare
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .unwrap_or_default();
            if copy_planes(&buffer, frame.yuv_mut()).is_ok() {
                let old = shared
                    .latest
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .replace(frame);
                if let Some(old) = old {
                    *shared.spare.lock().unwrap_or_else(PoisonError::into_inner) = Some(old);
                }
            }
        }
    }
);

impl Delegate {
    fn new(shared: Arc<Shared>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(shared);
        // SAFETY: plain NSObject initializer of a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

/// A camera capturing. Capture stops when it is dropped.
pub struct Camera {
    session: Retained<AVCaptureSession>,
    // Kept alive while the session delivers frames to them.
    _output: Retained<AVCaptureVideoDataOutput>,
    _delegate: Retained<Delegate>,
    _queue: dispatch2::DispatchRetained<DispatchQueue>,
    shared: Arc<Shared>,
}

// SAFETY: the session, output and delegate are only used to stop the
// capture when the camera is dropped, which AVFoundation allows from any
// thread; frames cross threads through the mutexes of `Shared`.
unsafe impl Send for Camera {}

impl Camera {
    /// Starts capturing from the camera `id`. Takes some time: call it off
    /// the render thread. The first time, macOS asks the user for access;
    /// the call then fails until access is granted.
    pub fn open(id: &str) -> Result<Self, DecodeError> {
        autoreleasepool(|_| {
            // SAFETY: reading an immutable framework constant.
            let video = unsafe { AVMediaTypeVideo }
                .ok_or_else(|| DecodeError::Platform("type de média vidéo indisponible".into()))?;
            // SAFETY: class method with a valid identifier string.
            let device = unsafe { AVCaptureDevice::deviceWithUniqueID(&NSString::from_str(id)) }
                .ok_or_else(|| DecodeError::Platform("caméra débranchée ou introuvable".into()))?;
            // SAFETY: class method with a valid media type.
            match unsafe { AVCaptureDevice::authorizationStatusForMediaType(video) } {
                AVAuthorizationStatus::Authorized => {}
                AVAuthorizationStatus::NotDetermined => {
                    let handler = block2::RcBlock::new(|_granted: objc2::runtime::Bool| {});
                    // SAFETY: valid media type and completion block.
                    unsafe {
                        AVCaptureDevice::requestAccessForMediaType_completionHandler(
                            video, &handler,
                        );
                    }
                    return Err(DecodeError::Platform(
                        "accès à la caméra demandé : autorisez-le, puis relancez le clip".into(),
                    ));
                }
                _ => {
                    return Err(DecodeError::Platform(
                        "accès à la caméra refusé : Réglages Système > Confidentialité et sécurité > Caméra".into(),
                    ));
                }
            }
            // SAFETY: valid device; the error is returned on failure.
            let input = unsafe { AVCaptureDeviceInput::deviceInputWithDevice_error(&device) }
                .map_err(|error| {
                    DecodeError::Platform(format!(
                        "caméra inutilisable : {}",
                        error.localizedDescription()
                    ))
                })?;
            // SAFETY: plain constructors.
            let session = unsafe { AVCaptureSession::new() };
            let output = unsafe { AVCaptureVideoDataOutput::new() };

            // SAFETY: CFString and NSString are toll-free bridged.
            let key: &NSString = unsafe {
                &*std::ptr::from_ref::<CFString>(kCVPixelBufferPixelFormatTypeKey)
                    .cast::<NSString>()
            };
            let format =
                NSNumber::numberWithUnsignedInt(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange);
            let value: &AnyObject = &format;
            let settings = NSDictionary::from_slices(&[key], &[value]);
            let shared = Arc::new(Shared::default());
            let delegate = Delegate::new(Arc::clone(&shared));
            let queue = DispatchQueue::new("prism.camera", None);
            // SAFETY: configuration of a session not running yet, with
            // inputs and outputs it accepts; the delegate and its serial
            // queue outlive the session (fields of the camera).
            unsafe {
                output.setVideoSettings(Some(&settings));
                output.setAlwaysDiscardsLateVideoFrames(true);
                output.setSampleBufferDelegate_queue(
                    Some(ProtocolObject::from_ref(&*delegate)),
                    Some(&queue),
                );
                if !session.canAddInput(&input) || !session.canAddOutput(&output) {
                    return Err(DecodeError::Platform(
                        "caméra déjà prise ou incompatible".into(),
                    ));
                }
                session.addInput(&input);
                session.addOutput(&output);
                session.startRunning();
            }
            Ok(Self {
                session,
                _output: output,
                _delegate: delegate,
                _queue: queue,
                shared,
            })
        })
    }

    /// The newest frame captured since the last call, if any.
    pub fn take_frame(&self) -> Option<Frame> {
        self.shared
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Gives back a frame taken, to be filled again.
    pub fn recycle(&self, frame: Frame) {
        *self
            .shared
            .spare
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(frame);
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        // SAFETY: stops the session this camera owns.
        unsafe { self.session.stopRunning() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cameras_are_listed_without_asking_for_access() {
        // Listing needs no permission; there may be no camera at all.
        for camera in cameras() {
            assert!(!camera.id.is_empty());
        }
    }
}
