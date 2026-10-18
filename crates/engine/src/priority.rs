//! Scheduling priority of the engine threads.
//!
//! On macOS, the render thread becomes a real-time thread, as audio and
//! display threads are: it declares how much work it does in each frame
//! period, and the scheduler wakes it on time even when other processes keep
//! every core busy. Should the system refuse, it keeps the quality-of-service
//! class of user interfaces. Decoding threads ask for the class just below.
//!
//! On Windows, the render thread gets the time-critical priority and the
//! system timer a 1 ms resolution, without which sleeps last up to 15.6 ms;
//! decoding threads get the priority above normal.

// A few system calls. Each block states why it is sound.
#![allow(unsafe_code)]

use std::time::Duration;

/// Called at the start of the render thread, and again when the frame
/// period changes. Returns whether the thread runs in real time.
pub(crate) fn render_thread(period: Duration) -> bool {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: changes the class of the calling thread only; the arguments
        // are a valid class and the relative priority 0.
        unsafe {
            libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0);
        }
        real_time(period)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = period;
        use windows_sys::Win32::Media::{TIMERR_NOERROR, timeBeginPeriod};
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
        };
        // SAFETY: plain calls with constant arguments; the pseudo handle of
        // the current thread needs no closing. The timer resolution holds
        // until the process ends.
        unsafe {
            let timer = timeBeginPeriod(1) == TIMERR_NOERROR;
            let priority =
                SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) != 0;
            timer && priority
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = period;
        false
    }
}

/// `mach_timebase_info_data_t` of `<mach/mach_time.h>`: the system clock
/// ticks `numer / denom` nanoseconds at a time. Declared here because the
/// libc crate deprecates its own binding.
#[cfg(target_os = "macos")]
#[repr(C)]
struct Timebase {
    numer: u32,
    denom: u32,
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn mach_timebase_info(info: *mut Timebase) -> libc::kern_return_t;
}

/// Real-time scheduling for the calling thread: up to a third of each
/// `period` of computation, done within half of it.
#[cfg(target_os = "macos")]
fn real_time(period: Duration) -> bool {
    let mut timebase = Timebase { numer: 0, denom: 0 };
    // SAFETY: writes the two fields of `timebase`, laid out as the C
    // structure.
    let status = unsafe { mach_timebase_info(&raw mut timebase) };
    if status != libc::KERN_SUCCESS || timebase.numer == 0 {
        return false;
    }
    // Durations in ticks of the system clock.
    let ticks = |duration: Duration| {
        let ticks = duration.as_nanos() * u128::from(timebase.denom) / u128::from(timebase.numer);
        u32::try_from(ticks).unwrap_or(u32::MAX)
    };
    let mut policy = libc::thread_time_constraint_policy {
        period: ticks(period),
        computation: ticks(period / 3),
        constraint: ticks(period / 2),
        preemptible: 1,
    };
    // SAFETY: `policy` is the structure the flavor expects, with its size in
    // integers; the port names the calling thread and needs no release.
    let status = unsafe {
        libc::thread_policy_set(
            libc::pthread_mach_thread_np(libc::pthread_self()),
            libc::THREAD_TIME_CONSTRAINT_POLICY as libc::thread_policy_flavor_t,
            (&raw mut policy).cast(),
            libc::THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        )
    };
    status == libc::KERN_SUCCESS
}

/// Called at the start of each decoding thread.
pub(crate) fn decode_thread() {
    #[cfg(target_os = "macos")]
    // SAFETY: as above.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INITIATED, 0);
    }
    #[cfg(target_os = "windows")]
    // SAFETY: as above.
    unsafe {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL,
        };
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
    }
}
