//! The Prism application: the control window, with the egui interface and
//! the program preview, and full-screen outputs on the other screens.
//!
//! The main thread handles window events and draws the interface. The
//! render thread of the engine plays the clips and presents the program to
//! the output window, so a busy interface never makes the output stutter.

mod control;
mod options;

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use prism_audio::{AudioInput, AudioOutput, Mixer, Volume};
use prism_control::{ControlMessage, MidiInputs};
use prism_engine::{Command, Engine};
use prism_output::{Fit, OutputShared, OutputSurface};
use prism_render::{Gpu, wgpu};
use prism_ui::{
    Action, AudioStatus, ControlPanel, FrameInfo, MidiStatus, Preferences, Screen, Shortcut,
};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowId};

use crate::control::ControlWindow;
use crate::options::{Options, USAGE};

/// How often the list of screens is read again.
const SCREEN_REFRESH: Duration = Duration::from_secs(2);

/// After the last input, the interface redraws at the screen rate this
/// long, then at [`IDLE_FRAME`] intervals.
const ACTIVE_FOR: Duration = Duration::from_millis(1500);

/// Interval between redraws of an idle interface (30 per second).
const IDLE_FRAME: Duration = Duration::from_millis(33);

fn main() -> ExitCode {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("erreur : {message}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    if options.help {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let (document, missing) = match options.document() {
        Ok(result) => result,
        Err(message) => {
            eprintln!("erreur : {message}");
            return ExitCode::FAILURE;
        }
    };

    let mut builder = EventLoop::<UserEvent>::with_user_event();
    // Without the default menu, Cmd+Q reaches the interface, which asks
    // about unsaved changes instead of quitting at once.
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_default_menu(false);
    }
    let event_loop = match builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("erreur : {error}");
            return ExitCode::FAILURE;
        }
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    let preferences_path = Preferences::default_path();
    let preferences = preferences_path
        .as_deref()
        .map(Preferences::load)
        .unwrap_or_default();
    // Syphon servers of other applications announce themselves on the run
    // loop of the main thread.
    #[cfg(target_os = "macos")]
    prism_io_video::start_syphon_directory();
    let (control_sender, control_messages) = std::sync::mpsc::channel();
    let proxy = event_loop.create_proxy();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = proxy.send_event(UserEvent::Control);
    });
    let mut app = App {
        midi: Midi {
            inputs: MidiInputs::new(control_sender.clone(), Arc::clone(&wake)),
            outputs: prism_control::MidiOutputs::default(),
            sender: control_sender,
            wake,
            osc_input: None,
            osc_wanted: None,
            osc_input_error: None,
            osc_output: None,
            osc_output_error: None,
            messages: control_messages,
            status: MidiStatus::default(),
            checked: None,
        },
        midi_disabled_seen: Vec::new(),
        midi_feedback_seen: true,
        output_screen: options.output_screen,
        panel: ControlPanel::new(document, missing).with_preferences(preferences, preferences_path),
        running: None,
        error: None,
        output_modifiers: ModifiersState::empty(),
        last_input: Instant::now(),
    };
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("erreur : {error}");
        return ExitCode::FAILURE;
    }
    match app.error {
        Some(message) => {
            eprintln!("erreur : {message}");
            ExitCode::FAILURE
        }
        None => ExitCode::SUCCESS,
    }
}

/// Message of an error and of all its causes.
fn describe(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(" : ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

/// The program image published over Syphon, as an output of the engine.
#[cfg(target_os = "macos")]
struct SyphonOutput(prism_io_video::SyphonServer);

#[cfg(target_os = "macos")]
impl prism_engine::Output for SyphonOutput {
    fn present(&mut self, gpu: &prism_render::Gpu, program: &prism_engine::ProgramOutput) {
        let size = program.resolution();
        self.0
            .publish(gpu, program.view(), (size.width, size.height));
    }
}

/// The program image published as an NDI source, as an output of the
/// engine.
#[cfg(feature = "ndi")]
struct NdiOutput(prism_io_video::NdiSender);

#[cfg(feature = "ndi")]
impl prism_engine::Output for NdiOutput {
    fn present(&mut self, gpu: &prism_render::Gpu, program: &prism_engine::ProgramOutput) {
        let size = program.resolution();
        self.0
            .publish(gpu, program.view(), (size.width, size.height));
    }
}

/// Events sent to the event loop from other threads.
#[derive(Debug)]
enum UserEvent {
    /// A controller sent messages.
    Control,
}

/// MIDI inputs, OSC input and output, and the messages they sent.
struct Midi {
    inputs: MidiInputs,
    outputs: prism_control::MidiOutputs,
    sender: std::sync::mpsc::Sender<ControlMessage>,
    wake: Arc<dyn Fn() + Send + Sync>,
    osc_input: Option<prism_control::OscInput>,
    /// Port asked for when the OSC input was last started or failed.
    osc_wanted: Option<u16>,
    osc_input_error: Option<String>,
    osc_output: Option<(String, prism_control::OscOutput)>,
    osc_output_error: Option<String>,
    messages: std::sync::mpsc::Receiver<ControlMessage>,
    status: MidiStatus,
    /// Last time the inputs present were looked at.
    checked: Option<Instant>,
}

/// How often the MIDI inputs present are looked at, to connect new ones.
const MIDI_REFRESH: Duration = Duration::from_secs(2);

impl Midi {
    /// Connects the inputs the preferences accept, every few seconds.
    fn refresh(&mut self, preferences: &Preferences, force: bool) {
        if !force
            && self
                .checked
                .is_some_and(|checked| checked.elapsed() < MIDI_REFRESH)
        {
            return;
        }
        self.checked = Some(Instant::now());
        let disabled = &preferences.midi_disabled;
        self.status.errors = self
            .inputs
            .sync(|name| !disabled.iter().any(|off| off == name));
        self.status.ports = prism_control::midi_ports();
        self.status.connected = self
            .inputs
            .connected()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let feedback = if preferences.midi_feedback {
            self.status.connected.clone()
        } else {
            Vec::new()
        };
        self.outputs.sync(&feedback);
    }

    /// Starts, moves or stops the OSC input and output as the preferences
    /// say, and sends the values of `composition` that changed, over OSC
    /// and back to the MIDI controllers.
    fn follow_osc(&mut self, preferences: &Preferences, composition: &prism_core::Composition) {
        let wanted = preferences.osc_input.then_some(preferences.osc_port);
        if wanted != self.osc_wanted {
            self.osc_wanted = wanted;
            self.osc_input = None;
            self.osc_input_error = None;
            if let Some(port) = wanted {
                match prism_control::OscInput::start(
                    port,
                    self.sender.clone(),
                    Arc::clone(&self.wake),
                ) {
                    Ok(input) => self.osc_input = Some(input),
                    Err(error) => {
                        self.osc_input_error = Some(format!("Port OSC {port} : {error}"));
                    }
                }
            }
        }
        if !preferences.osc_output {
            self.osc_output = None;
            self.osc_output_error = None;
        } else if self
            .osc_output
            .as_ref()
            .is_none_or(|(target, _)| *target != preferences.osc_target)
        {
            match prism_control::OscOutput::new(&preferences.osc_target) {
                Ok(output) => {
                    self.osc_output = Some((preferences.osc_target.clone(), output));
                    self.osc_output_error = None;
                }
                Err(error) => {
                    self.osc_output = None;
                    self.osc_output_error = Some(format!(
                        "Envoi OSC vers {} : {error}",
                        preferences.osc_target
                    ));
                }
            }
        }
        self.outputs
            .send_changes(&prism_control::midi_feedback(composition));
        if let Some((_, output)) = &mut self.osc_output
            && let Err(error) = output.send_changes(&prism_control::osc_feedback(composition))
        {
            self.osc_output_error = Some(format!("Envoi OSC : {error}"));
        }
        self.status.osc_port = self.osc_input.as_ref().map(prism_control::OscInput::port);
        self.status.osc_errors = self
            .osc_input_error
            .iter()
            .chain(&self.osc_output_error)
            .cloned()
            .collect();
    }
}

struct App {
    midi: Midi,
    /// MIDI inputs turned off when they were last connected.
    midi_disabled_seen: Vec<String>,
    /// Whether MIDI feedback was on when the outputs were last connected.
    midi_feedback_seen: bool,
    /// Screen, numbered from 1, for the output at start.
    output_screen: Option<usize>,
    panel: ControlPanel,
    running: Option<Running>,
    error: Option<String>,
    /// Modifier keys held over the output window.
    output_modifiers: ModifiersState,
    /// Last input on the control window.
    last_input: Instant,
}

/// The full-screen output window, presented by the engine.
struct OutputWindow {
    id: u64,
    screen: usize,
    window: Arc<Window>,
    shared: Arc<OutputShared>,
    /// Keeps the screens awake while the output is open.
    keep_awake: Option<std::process::Child>,
}

impl Drop for OutputWindow {
    fn drop(&mut self) {
        if let Some(child) = &mut self.keep_awake {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Keeps the displays awake until the child is stopped or Prism exits.
#[cfg(target_os = "macos")]
fn keep_displays_awake() -> Option<std::process::Child> {
    std::process::Command::new("/usr/bin/caffeinate")
        .args(["-d", "-i", "-w", &std::process::id().to_string()])
        .spawn()
        .ok()
}

#[cfg(not(target_os = "macos"))]
fn keep_displays_awake() -> Option<std::process::Child> {
    None
}

/// Everything that exists once the control window is open.
///
/// Field order matters: the engine stops, and drops the window surfaces it
/// presents to, before the windows themselves close.
struct Running {
    engine: Engine,
    gpu: Arc<Gpu>,
    control: ControlWindow,
    /// Full-screen outputs, at most one per screen.
    outputs: Vec<OutputWindow>,
    next_output: u64,
    monitors: Vec<MonitorHandle>,
    screens: Vec<Screen>,
    screens_read: Instant,
    sent_revision: u64,
    title: String,
    audio: AudioState,
}

/// The audio input and output as last asked for in the preferences.
#[derive(Default)]
struct AudioState {
    /// Whether an input was asked for, and which one.
    wanted: (bool, Option<String>),
    input: Option<AudioInput>,
    error: Option<String>,
    /// Whether the sound of the clips was asked for.
    sound_wanted: Option<bool>,
    /// Engine output of the Syphon server, while publishing.
    syphon: Option<u64>,
    /// Engine output of the NDI source, while publishing.
    #[cfg(feature = "ndi")]
    ndi: Option<u64>,
    output: Option<AudioOutput>,
    output_error: Option<String>,
    volume: Arc<Volume>,
}

impl Running {
    fn start(event_loop: &ActiveEventLoop, panel: &ControlPanel) -> Result<Self, Box<dyn Error>> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title(panel.title())
                    .with_inner_size(LogicalSize::new(1440.0, 900.0))
                    .with_min_inner_size(LogicalSize::new(960.0, 600.0)),
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
        let document = panel.document();
        let engine = Engine::start(Arc::clone(&gpu), document.composition().clone())?;
        let mut running = Self {
            engine,
            gpu,
            control,
            outputs: Vec::new(),
            next_output: 1,
            monitors: Vec::new(),
            screens: Vec::new(),
            screens_read: Instant::now(),
            sent_revision: document.revision(),
            title: panel.title(),
            audio: AudioState::default(),
        };
        running.read_screens(event_loop);
        running.follow_audio_preferences(panel.preferences());
        running.follow_syphon_preference(panel.preferences());
        running.follow_ndi_preference(panel.preferences(), document.composition());
        Ok(running)
    }

    /// Publishes as an NDI source, or stops, as the preferences say.
    fn follow_ndi_preference(
        &mut self,
        preferences: &Preferences,
        composition: &prism_core::Composition,
    ) {
        #[cfg(feature = "ndi")]
        match (preferences.ndi_output, self.audio.ndi) {
            (true, None) => {
                let id = self.next_output;
                self.next_output += 1;
                let sender =
                    prism_io_video::NdiSender::new("Prism", composition.frame_rate().as_f64());
                self.engine.send(Command::AddOutput {
                    id,
                    output: Box::new(NdiOutput(sender)),
                });
                self.audio.ndi = Some(id);
            }
            (false, Some(id)) => {
                self.engine.send(Command::RemoveOutput { id });
                self.audio.ndi = None;
            }
            _ => {}
        }
        #[cfg(not(feature = "ndi"))]
        let _ = (preferences, composition);
    }

    /// Publishes over Syphon, or stops, as the preferences say.
    fn follow_syphon_preference(&mut self, preferences: &Preferences) {
        #[cfg(target_os = "macos")]
        match (preferences.syphon_output, self.audio.syphon) {
            (true, None) => {
                let id = self.next_output;
                self.next_output += 1;
                self.engine.send(Command::AddOutput {
                    id,
                    output: Box::new(SyphonOutput(prism_io_video::SyphonServer::new("Prism"))),
                });
                self.audio.syphon = Some(id);
            }
            (false, Some(id)) => {
                self.engine.send(Command::RemoveOutput { id });
                self.audio.syphon = None;
            }
            _ => {}
        }
        #[cfg(not(target_os = "macos"))]
        let _ = preferences;
    }

    /// Starts, stops or changes the audio input and output to match the
    /// preferences.
    fn follow_audio_preferences(&mut self, preferences: &Preferences) {
        self.audio.volume.set(preferences.clip_sound_volume);
        if self.audio.sound_wanted != Some(preferences.clip_sound) {
            self.audio.sound_wanted = Some(preferences.clip_sound);
            self.audio.output = None;
            self.audio.output_error = None;
            let mut sender = None;
            if preferences.clip_sound {
                let (mixer, voices) = Mixer::new();
                match AudioOutput::start(mixer, Arc::clone(&self.audio.volume)) {
                    Ok(output) => {
                        self.audio.output = Some(output);
                        sender = Some(voices);
                    }
                    Err(error) => self.audio.output_error = Some(error.to_string()),
                }
            }
            self.engine.send(Command::SetSound(sender));
        }
        let levels = self.engine.audio_levels();
        levels.set_gain_db(preferences.audio_gain_db);
        let wanted = (preferences.audio_input, preferences.audio_device.clone());
        if wanted == self.audio.wanted {
            return;
        }
        // The old input stops before the new one starts.
        self.audio.input = None;
        self.audio.error = None;
        if wanted.0 {
            match AudioInput::start(wanted.1.as_deref(), levels) {
                Ok(input) => self.audio.input = Some(input),
                Err(error) => self.audio.error = Some(error.to_string()),
            }
        }
        self.audio.wanted = wanted;
    }

    fn audio_status(&self) -> AudioStatus {
        AudioStatus {
            device: self
                .audio
                .input
                .as_ref()
                .map(|input| input.name().to_owned()),
            error: self.audio.error.clone(),
            levels: self.engine.audio_levels().get(),
            output: self
                .audio
                .output
                .as_ref()
                .map(|output| output.name().to_owned()),
            output_error: self.audio.output_error.clone(),
        }
    }

    fn read_screens(&mut self, event_loop: &ActiveEventLoop) {
        let primary = event_loop.primary_monitor();
        self.monitors = event_loop.available_monitors().collect();
        self.screens = self
            .monitors
            .iter()
            .enumerate()
            .map(|(index, monitor)| {
                let size = monitor.size();
                Screen {
                    name: monitor
                        .name()
                        .unwrap_or_else(|| format!("Écran {}", index + 1)),
                    width: size.width,
                    height: size.height,
                    primary: Some(monitor) == primary.as_ref(),
                }
            })
            .collect();
        self.screens_read = Instant::now();
    }

    /// Screens for the output when none is chosen: those used last time
    /// that are still connected, otherwise the first screen that does not
    /// show the control window.
    fn default_screens(&self, preferred: &[String]) -> Vec<usize> {
        let remembered: Vec<usize> = preferred
            .iter()
            .filter_map(|name| self.screens.iter().position(|screen| &screen.name == name))
            .collect();
        if !remembered.is_empty() {
            return remembered;
        }
        let current = self.control.window.current_monitor();
        self.monitors
            .iter()
            .position(|monitor| Some(monitor) != current.as_ref())
            .or_else(|| (!self.monitors.is_empty()).then_some(0))
            .into_iter()
            .collect()
    }

    fn open_output(
        &mut self,
        event_loop: &ActiveEventLoop,
        screen: usize,
    ) -> Result<(), Box<dyn Error>> {
        if self.outputs.iter().any(|output| output.screen == screen) {
            return Ok(());
        }
        let monitor = self
            .monitors
            .get(screen)
            .cloned()
            .ok_or_else(|| format!("écran {} introuvable", screen + 1))?;
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Prism – sortie")
                    .with_fullscreen(Some(Fullscreen::Borderless(Some(monitor)))),
            )?,
        );
        window.set_cursor_visible(false);
        let surface = self.gpu.instance.create_surface(Arc::clone(&window))?;
        let size = window.inner_size();
        let output = OutputSurface::new(&self.gpu, surface, size.width, size.height, Fit::Contain)?;
        let id = self.next_output;
        self.next_output += 1;
        let shared = output.shared();
        self.engine.send(Command::AddOutput {
            id,
            output: Box::new(output),
        });
        // One process keeps every screen awake.
        let keep_awake = if self.outputs.is_empty() {
            keep_displays_awake()
        } else {
            None
        };
        self.outputs.push(OutputWindow {
            id,
            screen,
            window,
            shared,
            keep_awake,
        });
        // Keep the keyboard on the control window.
        self.control.window.focus_window();
        Ok(())
    }

    /// Closes the output on `screen`, if one is open there.
    fn close_output(&mut self, screen: usize) {
        let Some(index) = self
            .outputs
            .iter()
            .position(|output| output.screen == screen)
        else {
            return;
        };
        let mut output = self.outputs.remove(index);
        self.engine.send(Command::RemoveOutput { id: output.id });
        output.window.set_visible(false);
        // The process keeping the screens awake moves to an output left open.
        if let Some(next) = self.outputs.first_mut()
            && next.keep_awake.is_none()
        {
            next.keep_awake = output.keep_awake.take();
        }
    }

    fn close_outputs(&mut self) {
        let screens: Vec<usize> = self.outputs.iter().map(|output| output.screen).collect();
        for screen in screens {
            self.close_output(screen);
        }
    }

    /// Screens showing an output, in screen order.
    fn output_screens(&self) -> Vec<usize> {
        let mut screens: Vec<usize> = self.outputs.iter().map(|output| output.screen).collect();
        screens.sort_unstable();
        screens
    }
}

impl App {
    fn handle(&mut self, event_loop: &ActiveEventLoop, actions: Vec<Action>) {
        let Some(running) = &mut self.running else {
            return;
        };
        for action in actions {
            let result = match action {
                Action::ToggleScreen(screen) => {
                    if running.outputs.iter().any(|output| output.screen == screen) {
                        running.close_output(screen);
                        Ok(())
                    } else {
                        running.open_output(event_loop, screen)
                    }
                }
                Action::ToggleOutput => {
                    if running.outputs.is_empty() {
                        let preferred = &self.panel.preferences().output_screens;
                        running
                            .default_screens(preferred)
                            .into_iter()
                            .try_for_each(|screen| running.open_output(event_loop, screen))
                    } else {
                        running.close_outputs();
                        Ok(())
                    }
                }
                Action::CloseOutput => {
                    running.close_outputs();
                    Ok(())
                }
                Action::Engine(command) => {
                    running.engine.send(command);
                    Ok(())
                }
                Action::Quit => {
                    event_loop.exit();
                    Ok(())
                }
            };
            if let Err(error) = result {
                eprintln!("erreur : sortie impossible : {}", describe(error.as_ref()));
            }
        }
        running.follow_audio_preferences(self.panel.preferences());
        running.follow_syphon_preference(self.panel.preferences());
        running.follow_ndi_preference(
            self.panel.preferences(),
            self.panel.document().composition(),
        );
        // Remember the screens of the outputs for next time.
        let names: Vec<String> = running
            .output_screens()
            .into_iter()
            .filter_map(|screen| running.screens.get(screen))
            .map(|screen| screen.name.clone())
            .collect();
        if !names.is_empty() {
            self.panel.remember_output_screens(&names);
        }
        // Each output shows the screen of the stage set on its display, or
        // the whole program.
        let stage = &self.panel.document().composition().stage;
        for output in &running.outputs {
            let screen = running
                .screens
                .get(output.screen)
                .and_then(|display| stage.screen_on(&display.name))
                .map(prism_core::Screen::id);
            output.shared.set_screen(screen);
        }
        // The engine plays the composition as edited.
        let document = self.panel.document();
        if document.revision() != running.sent_revision {
            running
                .engine
                .set_composition(document.composition().clone());
            running.sent_revision = document.revision();
        }
        let title = self.panel.title();
        if title != running.title {
            running.control.window.set_title(&title);
            running.title = title;
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(running) = &mut self.running else {
            return;
        };
        if running.screens_read.elapsed() > SCREEN_REFRESH {
            running.read_screens(event_loop);
        }
        let status = running.engine.status();
        let program = running.engine.program();
        let output_screens = running.output_screens();
        let screens = running.screens.clone();
        let audio = running.audio_status();
        let preferences_changed = (
            &self.panel.preferences().midi_disabled,
            self.panel.preferences().midi_feedback,
        ) != (&self.midi_disabled_seen, self.midi_feedback_seen);
        self.midi
            .refresh(self.panel.preferences(), preferences_changed);
        self.midi.follow_osc(
            self.panel.preferences(),
            self.panel.document().composition(),
        );
        self.midi_disabled_seen = self.panel.preferences().midi_disabled.clone();
        self.midi_feedback_seen = self.panel.preferences().midi_feedback;
        let midi = self.midi.status.clone();
        let panel = &mut self.panel;
        let mut actions = Vec::new();
        running
            .control
            .draw(&running.gpu, Some(program), |ui, preview| {
                let frame = FrameInfo {
                    status: &status,
                    preview,
                    screens: &screens,
                    output_screens: &output_screens,
                    audio: audio.clone(),
                    midi: midi.clone(),
                };
                // egui may run the interface twice in one frame; only the first
                // pass sees the input, so keep the actions of every pass.
                actions.extend(panel.ui(ui, &frame));
            });
        self.handle(event_loop, actions);
        // Playheads and the preview move: keep drawing, at the screen rate
        // while the user acts or something animates, at 30 per second when
        // idle.
        if self.last_input.elapsed() < ACTIVE_FOR || self.panel.is_animating() {
            event_loop.set_control_flow(ControlFlow::Wait);
            if let Some(running) = &self.running {
                running.control.window.request_redraw();
            }
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + IDLE_FRAME));
        }
    }

    fn output_key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        let command = self.output_modifiers.super_key() || self.output_modifiers.control_key();
        // Learned keys act from the output window too.
        if !command
            && let Some(name) = key_name(key)
            && let Some(actions) = self.panel.press_key(&name)
        {
            self.handle(event_loop, actions);
            return;
        }
        let shortcut = match key {
            Key::Character(text) if command => (text.as_str() == "q").then_some(Shortcut::Quit),
            Key::Character(text) => Shortcut::from_text(text),
            Key::Named(NamedKey::Space) if !command => Some(Shortcut::StopAll),
            Key::Named(NamedKey::Escape) => Some(Shortcut::CloseOutput),
            _ => None,
        };
        if let Some(shortcut) = shortcut {
            self.panel.shortcut(shortcut);
            let actions = self.panel.take_actions();
            self.handle(event_loop, actions);
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Control => {
                let messages: Vec<ControlMessage> = self.midi.messages.try_iter().collect();
                if messages.is_empty() {
                    return;
                }
                let actions = self.panel.control(messages);
                self.handle(event_loop, actions);
                if let Some(running) = &self.running {
                    running.control.window.request_redraw();
                }
            }
        }
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        // Time for the next frame of an idle interface.
        if matches!(cause, StartCause::ResumeTimeReached { .. })
            && let Some(running) = &self.running
        {
            running.control.window.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        match Running::start(event_loop, &self.panel) {
            Ok(running) => {
                self.running = Some(running);
                if let Some(screen) = self.output_screen {
                    self.handle(event_loop, vec![Action::ToggleScreen(screen - 1)]);
                }
            }
            Err(error) => {
                self.error = Some(describe(error.as_ref()));
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(running) = &mut self.running else {
            return;
        };
        if running.control.window.id() == window_id {
            let repaint = running.control.on_event(&event);
            if !matches!(event, WindowEvent::RedrawRequested) {
                self.last_input = Instant::now();
            }
            match event {
                WindowEvent::CloseRequested => {
                    if self.panel.request_quit() {
                        event_loop.exit();
                    }
                }
                WindowEvent::RedrawRequested => self.redraw(event_loop),
                WindowEvent::DroppedFile(path) => {
                    let files: Vec<PathBuf> = vec![path];
                    self.panel.drop_files(&files);
                    self.handle(event_loop, Vec::new());
                }
                _ => {
                    if repaint {
                        running.control.window.request_redraw();
                    }
                }
            }
            return;
        }

        let Some(output) = running
            .outputs
            .iter()
            .find(|output| output.window.id() == window_id)
        else {
            return;
        };
        match event {
            WindowEvent::Resized(size) => output.shared.set_size(size.width, size.height),
            WindowEvent::Occluded(occluded) => output.shared.set_visible(!occluded),
            WindowEvent::CloseRequested => {
                let screen = output.screen;
                running.close_output(screen);
            }
            WindowEvent::ModifiersChanged(modifiers) => self.output_modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                self.output_key(event_loop, &event.logical_key);
            }
            _ => {}
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Stop the render thread before closing the windows it presents to.
        if let Some(running) = self.running.take() {
            let Running {
                engine,
                control,
                outputs,
                ..
            } = running;
            drop(engine);
            drop(outputs);
            drop(control);
        }
    }
}

/// Name of a key as the control window names it (see `egui::Key::name`).
fn key_name(key: &Key) -> Option<String> {
    match key {
        Key::Character(text) => Some(text.to_uppercase()),
        Key::Named(NamedKey::Space) => Some("Space".into()),
        Key::Named(NamedKey::ArrowUp) => Some("Up".into()),
        Key::Named(NamedKey::ArrowDown) => Some("Down".into()),
        Key::Named(NamedKey::ArrowLeft) => Some("Left".into()),
        Key::Named(NamedKey::ArrowRight) => Some("Right".into()),
        Key::Named(NamedKey::Enter) => Some("Enter".into()),
        Key::Named(NamedKey::Tab) => Some("Tab".into()),
        _ => None,
    }
}
