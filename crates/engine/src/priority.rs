//! Scheduling priority of the engine threads.
//!
//! On macOS, a thread's quality-of-service class tells the system how soon
//! to run it. The render thread asks for the class of user interfaces, so a
//! busy machine wakes it on time; decoding threads ask for the class just
//! below.

// One system call per thread. Each block states why it is sound.
#![allow(unsafe_code)]

/// Called at the start of the render thread.
pub(crate) fn render_thread() {
    #[cfg(target_os = "macos")]
    // SAFETY: changes the class of the calling thread only; the arguments
    // are a valid class and the relative priority 0.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0);
    }
}

/// Called at the start of each decoding thread.
pub(crate) fn decode_thread() {
    #[cfg(target_os = "macos")]
    // SAFETY: as above.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0);
    }
}
