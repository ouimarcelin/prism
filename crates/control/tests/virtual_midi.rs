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
    while start.elapsed() < Duration::from_secs(2) {
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
