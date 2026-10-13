//! The Prism application: a control window with the program preview, and a
//! full-screen output on a second screen.
//!
//! The main thread only handles window events and keyboard shortcuts. The
//! render thread of the engine plays the clips and presents the program to
//! every window, so a busy main thread never makes the output stutter.

mod options;

use std::error::Error;
use std::process::ExitCode;
use std::sync::Arc;

use prism_core::Composition;
use prism_engine::{Command, Engine};
use prism_output::{Fit, OutputShared, OutputSurface};
use prism_render::{Gpu, wgpu};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowId};

use crate::options::{Options, USAGE};

/// Identifier of the preview output in the engine.
const PREVIEW_OUTPUT: u64 = 0;

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
    let (composition, warnings) = match options.composition() {
        Ok(result) => result,
        Err(message) => {
            eprintln!("erreur : {message}");
            return ExitCode::FAILURE;
        }
    };
    for warning in warnings {
        eprintln!("attention : {warning}");
    }

    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("erreur : {error}");
            return ExitCode::FAILURE;
        }
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        options,
        document: composition,
        running: None,
        error: None,
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
    options: Options,
    /// The composition being played, edited here and sent to the engine.
    document: Composition,
    running: Option<Running>,
    error: Option<String>,
}

/// A window shown by the engine.
struct OutputWindow {
    id: u64,
    window: Arc<Window>,
    shared: Arc<OutputShared>,
}

/// Everything that exists once the first window is open.
///
/// Field order matters: the engine stops, and drops the window surfaces it
/// presents to, before the windows themselves close.
struct Running {
    engine: Engine,
    gpu: Arc<Gpu>,
    control: OutputWindow,
    output: Option<OutputWindow>,
    next_output: u64,
}

impl Running {
    fn start(
        event_loop: &ActiveEventLoop,
        composition: &Composition,
    ) -> Result<Self, Box<dyn Error>> {
        let control = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Prism")
                    .with_inner_size(LogicalSize::new(1280.0, 720.0)),
            )?,
        );
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
                event_loop.owned_display_handle(),
            ))
        });
        let surface = instance.create_surface(Arc::clone(&control))?;
        let gpu = Arc::new(pollster::block_on(Gpu::from_instance(
            &instance,
            Some(&surface),
        ))?);
        let info = gpu.adapter.get_info();
        println!("GPU : {} ({:?})", info.name, info.backend);

        let size = control.inner_size();
        let preview = OutputSurface::new(&gpu, surface, size.width, size.height, Fit::Contain)?;
        let shared = preview.shared();
        let engine = Engine::start(Arc::clone(&gpu), composition.clone())?;
        engine.send(Command::AddOutput {
            id: PREVIEW_OUTPUT,
            output: Box::new(preview),
        });

        println!("Écrans :");
        let primary = event_loop.primary_monitor();
        for (index, monitor) in event_loop.available_monitors().enumerate() {
            let size = monitor.size();
            println!(
                "  {}. {} ({} × {}){}",
                index + 1,
                monitor.name().unwrap_or_else(|| "écran".into()),
                size.width,
                size.height,
                if Some(&monitor) == primary.as_ref() {
                    ", principal"
                } else {
                    ""
                }
            );
        }

        Ok(Self {
            engine,
            gpu,
            control: OutputWindow {
                id: PREVIEW_OUTPUT,
                window: control,
                shared,
            },
            output: None,
            next_output: PREVIEW_OUTPUT + 1,
        })
    }

    fn window(&self, id: WindowId) -> Option<&OutputWindow> {
        std::iter::once(&self.control)
            .chain(self.output.as_ref())
            .find(|output| output.window.id() == id)
    }

    /// Screen for the output: the one asked for, otherwise the first screen
    /// that does not show the control window.
    fn output_monitor(
        &self,
        event_loop: &ActiveEventLoop,
        requested: Option<usize>,
    ) -> Option<MonitorHandle> {
        let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
        if let Some(screen) = requested {
            return monitors.get(screen - 1).cloned();
        }
        let current = self.control.window.current_monitor();
        monitors
            .iter()
            .find(|monitor| Some(*monitor) != current.as_ref())
            .cloned()
            .or(current)
    }

    fn open_output(
        &mut self,
        event_loop: &ActiveEventLoop,
        requested: Option<usize>,
    ) -> Result<(), Box<dyn Error>> {
        if self.output.is_some() {
            return Ok(());
        }
        let Some(monitor) = self.output_monitor(event_loop, requested) else {
            return Err(format!("écran {} introuvable", requested.unwrap_or(0)).into());
        };
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
        self.output = Some(OutputWindow { id, window, shared });
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
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: &dyn Error) {
        self.error = Some(describe(error));
        event_loop.exit();
    }

    /// Sends the edited composition to the engine.
    fn publish(&self) {
        if let Some(running) = &self.running {
            running.engine.set_composition(self.document.clone());
        }
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        match key {
            Key::Character(text) => match text.as_str() {
                "f" | "F" => self.toggle_output(event_loop),
                digit => {
                    let column = digit
                        .parse::<usize>()
                        .ok()
                        .filter(|n| *n >= 1)
                        .and_then(|n| self.document.columns().get(n - 1))
                        .map(|column| column.id());
                    if let Some(column) = column
                        && self.document.trigger_column(column).is_ok()
                    {
                        self.publish();
                    }
                }
            },
            Key::Named(NamedKey::Space) => {
                self.document.stop_all();
                self.publish();
            }
            Key::Named(NamedKey::Escape) => {
                if let Some(running) = &mut self.running {
                    running.close_output();
                }
            }
            _ => {}
        }
    }

    fn toggle_output(&mut self, event_loop: &ActiveEventLoop) {
        let Some(running) = &mut self.running else {
            return;
        };
        if running.output.is_some() {
            running.close_output();
        } else if let Err(error) = running.open_output(event_loop, None) {
            eprintln!("erreur : sortie impossible : {}", describe(error.as_ref()));
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        match Running::start(event_loop, &self.document) {
            Ok(mut running) => {
                if let Some(screen) = self.options.output_screen
                    && let Err(error) = running.open_output(event_loop, Some(screen))
                {
                    eprintln!("erreur : sortie impossible : {}", describe(error.as_ref()));
                }
                self.running = Some(running);
            }
            Err(error) => self.fail(event_loop, error.as_ref()),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(running) = &self.running else {
            return;
        };
        let Some(window) = running.window(window_id) else {
            return;
        };
        let is_control = window.id == running.control.id;
        match event {
            WindowEvent::CloseRequested => {
                if is_control {
                    event_loop.exit();
                } else if let Some(running) = &mut self.running {
                    running.close_output();
                }
            }
            WindowEvent::Resized(size) => window.shared.set_size(size.width, size.height),
            WindowEvent::Occluded(occluded) => window.shared.set_visible(!occluded),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                self.key(event_loop, &event.logical_key);
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
