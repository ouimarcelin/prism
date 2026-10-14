//! The Prism application: the control window, with the egui interface and
//! the program preview, and a full-screen output on a second screen.
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

use prism_engine::{Command, Engine};
use prism_output::{Fit, OutputShared, OutputSurface};
use prism_render::{Gpu, wgpu};
use prism_ui::{Action, ControlPanel, FrameInfo, Preferences, Screen, Shortcut};
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

    let mut builder = EventLoop::builder();
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
    let mut app = App {
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

struct App {
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
    output: Option<OutputWindow>,
    next_output: u64,
    monitors: Vec<MonitorHandle>,
    screens: Vec<Screen>,
    screens_read: Instant,
    sent_revision: u64,
    title: String,
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
            output: None,
            next_output: 1,
            monitors: Vec::new(),
            screens: Vec::new(),
            screens_read: Instant::now(),
            sent_revision: document.revision(),
            title: panel.title(),
        };
        running.read_screens(event_loop);
        Ok(running)
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

    /// Screen for the output when none is chosen: the one used last time,
    /// otherwise the first one that does not show the control window.
    fn default_screen(&self, preferred: Option<&str>) -> Option<usize> {
        if let Some(index) =
            preferred.and_then(|name| self.screens.iter().position(|screen| screen.name == name))
        {
            return Some(index);
        }
        let current = self.control.window.current_monitor();
        self.monitors
            .iter()
            .position(|monitor| Some(monitor) != current.as_ref())
            .or_else(|| (!self.monitors.is_empty()).then_some(0))
    }

    fn open_output(
        &mut self,
        event_loop: &ActiveEventLoop,
        screen: usize,
    ) -> Result<(), Box<dyn Error>> {
        if self
            .output
            .as_ref()
            .is_some_and(|output| output.screen == screen)
        {
            return Ok(());
        }
        self.close_output();
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
        self.output = Some(OutputWindow {
            id,
            screen,
            window,
            shared,
            keep_awake: keep_displays_awake(),
        });
        // Keep the keyboard on the control window.
        self.control.window.focus_window();
        Ok(())
    }

    fn close_output(&mut self) {
        if let Some(output) = self.output.take() {
            self.engine.send(Command::RemoveOutput { id: output.id });
            output.window.set_visible(false);
        }
    }
}

impl App {
    fn handle(&mut self, event_loop: &ActiveEventLoop, actions: Vec<Action>) {
        let Some(running) = &mut self.running else {
            return;
        };
        for action in actions {
            let result = match action {
                Action::OpenOutput(screen) => running.open_output(event_loop, screen),
                Action::ToggleOutput => {
                    let preferred = self.panel.preferences().output_screen.as_deref();
                    if running.output.is_some() {
                        running.close_output();
                        Ok(())
                    } else if let Some(screen) = running.default_screen(preferred) {
                        running.open_output(event_loop, screen)
                    } else {
                        Ok(())
                    }
                }
                Action::CloseOutput => {
                    running.close_output();
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
        // Remember the screen of the output for next time.
        if let Some(name) = running
            .output
            .as_ref()
            .and_then(|output| running.screens.get(output.screen))
            .map(|screen| screen.name.clone())
        {
            self.panel.remember_output_screen(&name);
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
        let output_screen = running.output.as_ref().map(|output| output.screen);
        let screens = running.screens.clone();
        let panel = &mut self.panel;
        let mut actions = Vec::new();
        running.control.draw(&running.gpu, program, |ui, preview| {
            let frame = FrameInfo {
                status: &status,
                preview,
                screens: &screens,
                output_screen,
            };
            actions = panel.ui(ui, &frame);
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

impl ApplicationHandler for App {
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
                    self.handle(event_loop, vec![Action::OpenOutput(screen - 1)]);
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
            .output
            .as_ref()
            .filter(|output| output.window.id() == window_id)
        else {
            return;
        };
        match event {
            WindowEvent::Resized(size) => output.shared.set_size(size.width, size.height),
            WindowEvent::Occluded(occluded) => output.shared.set_visible(!occluded),
            WindowEvent::CloseRequested => running.close_output(),
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
                output,
                ..
            } = running;
            drop(engine);
            drop(output);
            drop(control);
        }
    }
}
