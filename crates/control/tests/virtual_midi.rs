//! MIDI through virtual ports of the system: a controller made by a test
//! sends to the inputs Prism listens to, and receives its feedback.

#![cfg(target_os = "macos")]

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use midir::os::unix::{VirtualInput, VirtualOutput};
use midir::{MidiInput, MidiOutput};
use prism_control::{ControlMessage, MidiInputs, MidiOutputs, midi_ports};
use prism_core::ControlSource;

/// Waits up to two seconds for `done`.
fn wait_for(mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn a_virtual_controller_is_heard_and_answered() {
    let name = format!("Prism test {}", std::process::id());
    let Ok(output) = MidiOutput::new("prism-test") else {
        eprintln!("no MIDI on this system: skipped");
        return;
    };
    let mut controller = output.create_virtual(&name).unwrap();
    let received = Arc::new(Mutex::new(Vec::new()));
    let feedback = Arc::clone(&received);
    let _destination = MidiInput::new("prism-test")
        .unwrap()
        .create_virtual(
            &name,
            move |_, bytes, ()| feedback.lock().unwrap().push(bytes.to_vec()),
            (),
        )
        .unwrap();
    assert!(wait_for(|| midi_ports().contains(&name)));

    let (sender, messages) = mpsc::channel();
    let mut inputs = MidiInputs::new(sender, Arc::new(|| {}));
    let errors = inputs.sync(|port| port == name);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(inputs.connected(), [name.as_str()]);

    controller.send(&[0x90, 36, 127]).unwrap();
    let message: ControlMessage = messages.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        message.source,
        ControlSource::MidiNote {
            channel: 0,
            note: 36
        }
    );
    assert!(message.pressed);

    let mut outputs = MidiOutputs::default();
    outputs.sync(std::slice::from_ref(&name));
    outputs.send_changes(&[[0x90, 36, 127]]);
    assert!(wait_for(|| !received.lock().unwrap().is_empty()));
    assert_eq!(received.lock().unwrap()[0], [0x90, 36, 127]);
}

#[test]
fn a_midi_clock_gives_the_tempo() {
    let name = format!("Prism clock {}", std::process::id());
    let Ok(output) = MidiOutput::new("prism-test") else {
        eprintln!("no MIDI on this system: skipped");
        return;
    };
    let mut sender = output.create_virtual(&name).unwrap();
    assert!(wait_for(|| midi_ports().contains(&name)));
    let (messages, _) = mpsc::channel();
    let mut inputs = MidiInputs::new(messages, Arc::new(|| {}));
    assert!(inputs.sync(|port| port == name).is_empty());
    assert_eq!(inputs.clock().bpm, None);
    sender.send(&[0xFA]).unwrap();
    // 125 BPM: a pulse every 20 ms, sent until the tempo is read, so a
    // slow machine never sees the clock stop.
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let still = Arc::clone(&running);
    let pulses = std::thread::spawn(move || {
        let start = Instant::now();
        let mut pulse = 1u32;
        while still.load(std::sync::atomic::Ordering::Relaxed) && pulse < 1000 {
            sender.send(&[0xF8]).unwrap();
            let next = start + Duration::from_millis(20) * pulse;
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
            pulse += 1;
        }
    });
    let read = wait_for(|| {
        let clock = inputs.clock();
        clock.starts == 1 && clock.bpm.is_some_and(|bpm| (bpm - 125.0).abs() < 3.0)
    });
    running.store(false, std::sync::atomic::Ordering::Relaxed);
    pulses.join().unwrap();
    assert!(read, "{:?}", inputs.clock().bpm);
}

#[test]
fn midi_timecode_is_read_and_runs_on() {
    let name = format!("Prism timecode {}", std::process::id());
    let Ok(output) = MidiOutput::new("prism-test") else {
        eprintln!("no MIDI on this system: skipped");
        return;
    };
    let mut sender = output.create_virtual(&name).unwrap();
    assert!(wait_for(|| midi_ports().contains(&name)));
    let (messages, _) = mpsc::channel();
    let mut inputs = MidiInputs::new(messages, Arc::new(|| {}));
    assert!(inputs.sync(|port| port == name).is_empty());
    // Full frame: 01:00:00:00 at 25 frames per second.
    sender
        .send(&[0xF0, 0x7F, 0x7F, 0x01, 0x01, 0x20 | 1, 0, 0, 0, 0xF7])
        .unwrap();
    assert!(wait_for(|| inputs.clock().timecode.is_some()));
    let timecode = inputs.clock().timecode.unwrap();
    assert_eq!(
        (timecode.hours, timecode.minutes, timecode.seconds),
        (1, 0, 0)
    );
    assert!(timecode.frames < 5, "{timecode}");
    // Silent for long: no longer running.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(inputs.clock().timecode, None);
}
