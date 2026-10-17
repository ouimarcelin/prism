//! A MIDI controller on the screen, for playing Prism without one.
//!
//! ```text
//! cargo run --release -p prism-app --example midi-pad
//! ```
//!
//! The window creates a MIDI input and a MIDI output of the system, both
//! named « Pad virtuel », which Prism lists like those of a real
//! controller: learn its pads and faders (menu Contrôle > Apprentissage
//! MIDI), then play. Prism's feedback lights the pads and moves the faders.
//!
//! Layout, on MIDI channel 1: 32 pads (notes 36 to 67, four rows of eight),
//! 8 buttons (notes 68 to 75), 8 faders (CC 1 to 8) and a crossfader
//! (CC 9). Keys `1` to `8` press the pads of the first row while held, so
//! a learned clip can be launched without the mouse.

use std::process::ExitCode;

#[cfg(unix)]
#[path = "../src/control.rs"]
mod control;

fn main() -> ExitCode {
    #[cfg(unix)]
    return pad::run();
    #[cfg(not(unix))]
    {
        eprintln!("erreur : ports MIDI virtuels absents de ce système");
        ExitCode::FAILURE
    }
}

/// Everything but `main`: virtual MIDI ports exist on macOS and Linux only.
#[cfg(unix)]
mod pad {
    use std::error::Error;
    use std::process::ExitCode;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use midir::os::unix::{VirtualInput, VirtualOutput};
    use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
    use prism_render::{Gpu, wgpu};
    use winit::application::ApplicationHandler;
    use winit::dpi::LogicalSize;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::{Window, WindowId};

    use super::control::ControlWindow;

    /// Name of the virtual input and output, as Prism lists them.
    const PORT_NAME: &str = "Pad virtuel";
    const ROWS: usize = 4;
    const COLUMNS: usize = 8;
    const FIRST_PAD: u8 = 36;
    const FIRST_BUTTON: u8 = FIRST_PAD + (ROWS * COLUMNS) as u8;
    const FADERS: usize = 8;
    const FIRST_FADER: u8 = 1;
    const CROSSFADER: u8 = FIRST_FADER + FADERS as u8;
    /// Redraw rate, for the feedback received.
    const FRAME: Duration = Duration::from_millis(33);

    /// State of the controls: what is held, what the feedback lit, where the
    /// faders are.
    struct Surface {
        /// Velocity of the feedback note of each note number.
        lit: [u8; 128],
        /// Whether each note is held down.
        held: [bool; 128],
        /// Value of each control change number.
        values: [u8; 128],
    }

    impl Surface {
        fn new() -> Self {
            Self {
                lit: [0; 128],
                held: [false; 128],
                values: [0; 128],
            }
        }

        /// Holds or releases `note`; the message to send if it changed.
        fn hold(&mut self, note: u8, down: bool) -> Option<[u8; 3]> {
            let held = &mut self.held[usize::from(note)];
            if *held == down {
                return None;
            }
            *held = down;
            Some(if down {
                [0x90, note, 127]
            } else {
                [0x80, note, 0]
            })
        }

        /// Moves control `number` to `value`; the message to send if it moved.
        fn set(&mut self, number: u8, value: u8) -> Option<[u8; 3]> {
            let current = &mut self.values[usize::from(number)];
            if *current == value {
                return None;
            }
            *current = value;
            Some([0xB0, number, value])
        }

        /// Takes a message from Prism: notes light the pads, control changes
        /// move the faders. Only channel 1 is read.
        fn feedback(&mut self, bytes: &[u8]) {
            let [status, number, value, ..] = *bytes else {
                return;
            };
            let (number, value) = (usize::from(number & 0x7F), value & 0x7F);
            match status {
                0x90 => self.lit[number] = value,
                0x80 => self.lit[number] = 0,
                0xB0 => self.values[number] = value,
                _ => {}
            }
        }
    }

    /// The virtual ports and the controls.
    struct Controller {
        output: MidiOutputConnection,
        _input: MidiInputConnection<()>,
        surface: Arc<Mutex<Surface>>,
    }

    impl Controller {
        fn open() -> Result<Self, Box<dyn Error>> {
            let surface = Arc::new(Mutex::new(Surface::new()));
            let output = MidiOutput::new("Prism pad")?
                .create_virtual(PORT_NAME)
                .map_err(|error| error.to_string())?;
            let feedback = Arc::clone(&surface);
            let input = MidiInput::new("Prism pad")?
                .create_virtual(
                    PORT_NAME,
                    move |_, bytes, ()| {
                        if let Ok(mut surface) = feedback.lock() {
                            surface.feedback(bytes);
                        }
                    },
                    (),
                )
                .map_err(|error| error.to_string())?;
            Ok(Self {
                output,
                _input: input,
                surface,
            })
        }

        fn send(&mut self, message: Option<[u8; 3]>) {
            if let Some(message) = message {
                let _ = self.output.send(&message);
            }
        }

        fn ui(&mut self, ui: &mut egui::Ui) {
            let mut messages = Vec::new();
            {
                let mut surface = self
                    .surface
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                ui.heading(format!("{PORT_NAME} – MIDI canal 1"));
                ui.label("Dans Prism : Contrôle > Apprentissage MIDI, cliquer une commande, puis un pad ou un fader ici.");
                ui.add_space(8.0);

                // Keys 1 to 8 hold the pads of the first row.
                let keys = [
                    egui::Key::Num1,
                    egui::Key::Num2,
                    egui::Key::Num3,
                    egui::Key::Num4,
                    egui::Key::Num5,
                    egui::Key::Num6,
                    egui::Key::Num7,
                    egui::Key::Num8,
                ];
                let keys_down: Vec<bool> = keys
                    .iter()
                    .map(|key| ui.input(|input| input.key_down(*key)))
                    .collect();

                for row in 0..ROWS {
                    ui.horizontal(|ui| {
                        for (column, key_down) in keys_down.iter().enumerate() {
                            let note = FIRST_PAD + (row * COLUMNS + column) as u8;
                            let key = row == 0 && *key_down;
                            let label = if row == 0 {
                                format!("{note}\n[{}]", column + 1)
                            } else {
                                note.to_string()
                            };
                            let down = pad(ui, &label, surface.lit[usize::from(note)], 64.0) || key;
                            messages.push(surface.hold(note, down));
                        }
                    });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for index in 0..COLUMNS {
                        let note = FIRST_BUTTON + index as u8;
                        let down = pad(ui, &note.to_string(), surface.lit[usize::from(note)], 32.0);
                        messages.push(surface.hold(note, down));
                    }
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    for index in 0..FADERS {
                        let number = FIRST_FADER + index as u8;
                        let mut value = surface.values[usize::from(number)];
                        ui.vertical(|ui| {
                            // One fader under each column of pads.
                            ui.set_width(64.0);
                            ui.spacing_mut().slider_width = 160.0;
                            let changed = ui
                                .add(
                                    egui::Slider::new(&mut value, 0..=127)
                                        .vertical()
                                        .show_value(false),
                                )
                                .changed();
                            ui.label(format!("CC {number}"));
                            if changed {
                                messages.push(surface.set(number, value));
                            }
                        });
                    }
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let mut value = surface.values[usize::from(CROSSFADER)];
                    ui.spacing_mut().slider_width = 480.0;
                    ui.label("A");
                    let changed = ui
                        .add(egui::Slider::new(&mut value, 0..=127).show_value(false))
                        .changed();
                    ui.label(format!("B   (CC {CROSSFADER})"));
                    if changed {
                        messages.push(surface.set(CROSSFADER, value));
                    }
                });
            }
            for message in messages {
                self.send(message);
            }
        }
    }

    /// A pad, lit by the feedback velocity; returns whether it is held down.
    fn pad(ui: &mut egui::Ui, label: &str, lit: u8, height: f32) -> bool {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(64.0, height), egui::Sense::click_and_drag());
        let down = response.is_pointer_button_down_on();
        let light = f32::from(lit) / 127.0;
        let fill = if down {
            egui::Color32::from_rgb(230, 230, 230)
        } else {
            egui::Color32::from_rgb(
                (40.0 + 200.0 * light) as u8,
                (40.0 + 140.0 * light) as u8,
                (46.0 - 20.0 * light) as u8,
            )
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 6.0, fill);
        let text = if down || light > 0.5 {
            egui::Color32::BLACK
        } else {
            egui::Color32::GRAY
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            text,
        );
        down
    }

    struct App {
        controller: Controller,
        window: Option<(Arc<Gpu>, ControlWindow)>,
        error: Option<String>,
    }

    impl App {
        fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Prism – pad MIDI virtuel")
                        .with_inner_size(LogicalSize::new(600.0, 640.0)),
                )?,
            );
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::PRIMARY,
                ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                    event_loop.owned_display_handle(),
                ))
            });
            let surface = instance.create_surface(Arc::clone(&window))?;
            let gpu = Arc::new(pollster::block_on(Gpu::from_instance(
                &instance,
                Some(&surface),
            ))?);
            let control = ControlWindow::new(&gpu, window, surface)?;
            self.window = Some((gpu, control));
            Ok(())
        }
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_none()
                && let Err(error) = self.open(event_loop)
            {
                self.error = Some(error.to_string());
                event_loop.exit();
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            let Some((gpu, window)) = &mut self.window else {
                return;
            };
            window.on_event(&event);
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => {
                    let controller = &mut self.controller;
                    window.draw(gpu, &[], |ui, _| {
                        egui::Frame::central_panel(ui.style()).show(ui, |ui| controller.ui(ui));
                    });
                }
                _ => window.window.request_redraw(),
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            // Feedback arrives on its own thread: redraw regularly to show it.
            if let Some((_, window)) = &self.window {
                window.window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
        }
    }

    pub(crate) fn run() -> ExitCode {
        let controller = match Controller::open() {
            Ok(controller) => controller,
            Err(error) => {
                eprintln!("erreur : ports MIDI virtuels impossibles : {error}");
                return ExitCode::FAILURE;
            }
        };
        let result = EventLoop::new()
            .map_err(|error| error.to_string())
            .and_then(|event_loop| {
                let mut app = App {
                    controller,
                    window: None,
                    error: None,
                };
                event_loop
                    .run_app(&mut app)
                    .map_err(|error| error.to_string())?;
                app.error.map_or(Ok(()), Err)
            });
        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("erreur : {error}");
                ExitCode::FAILURE
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn pads_send_once_per_press_and_release() {
            let mut surface = Surface::new();
            assert_eq!(surface.hold(36, true), Some([0x90, 36, 127]));
            assert_eq!(surface.hold(36, true), None);
            assert_eq!(surface.hold(36, false), Some([0x80, 36, 0]));
            assert_eq!(surface.hold(36, false), None);
        }

        #[test]
        fn faders_send_only_moves() {
            let mut surface = Surface::new();
            assert_eq!(surface.set(1, 64), Some([0xB0, 1, 64]));
            assert_eq!(surface.set(1, 64), None);
        }

        #[test]
        fn feedback_lights_pads_and_moves_faders() {
            let mut surface = Surface::new();
            surface.feedback(&[0x90, 36, 127]);
            surface.feedback(&[0xB0, 3, 100]);
            assert_eq!(surface.lit[36], 127);
            assert_eq!(surface.values[3], 100);
            surface.feedback(&[0x80, 36, 64]);
            assert_eq!(surface.lit[36], 0);
            // Another channel is not this controller's.
            surface.feedback(&[0x91, 37, 127]);
            assert_eq!(surface.lit[37], 0);
            // A moved fader is not sent back as a move.
            assert_eq!(surface.set(3, 100), None);
        }
    }
}
