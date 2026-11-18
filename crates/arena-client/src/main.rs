//! The window of the client: it moves the events of the window to the view, and draws each frame.
//!
//! `arena-client --frames <n>` ends after n frames, as the gate does under a virtual display.

use std::num::ParseIntError;
use std::sync::Arc;

use arena_client::font::{self, SystemFont};
use arena_client::paint::Painter;
use arena_client::screen::Screen;
use arena_client::view::View;
use arena_sim::map::FIRST;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Debug, thiserror::Error)]
enum Error {
  #[error("the client takes no argument, or --frames <n>")]
  Usage,
  #[error("--frames takes a whole number of frames")]
  Frames(#[from] ParseIntError),
  #[error(transparent)]
  Font(#[from] font::Error),
  #[error("the loop of the events of the window fails")]
  Events(#[from] winit::error::EventLoopError),
  #[error("the window cannot open")]
  Window(#[from] winit::error::OsError),
  #[error("the window gives no surface to draw on")]
  Surface(#[from] wgpu::CreateSurfaceError),
  #[error("no graphics adapter can draw on the window")]
  Adapter(#[from] wgpu::RequestAdapterError),
  #[error("the graphics adapter gives no device")]
  Device(#[from] wgpu::RequestDeviceError),
  #[error("the graphics adapter cannot draw on the surface of the window")]
  Format,
}

struct Client {
  frames: Option<u32>,
  font: Option<SystemFont>,
  open: Option<Open>,
  fault: Option<Error>,
}

struct Open {
  window: Arc<Window>,
  surface: wgpu::Surface<'static>,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  format: wgpu::TextureFormat,
  screen: Screen<SystemFont>,
  view: View,
}

fn main() -> Result<(), Error> {
  let frames = frames(&std::env::args().skip(1).collect::<Vec<_>>())?;
  let font = SystemFont::find()?;
  eprintln!("arena draws its characters with the font {}", font.name());
  let events = EventLoop::new()?;
  let mut client = Client { frames, font: Some(font), open: None, fault: None };
  events.run_app(&mut client)?;
  client.fault.map_or(Ok(()), Err)
}

fn frames(arguments: &[String]) -> Result<Option<u32>, Error> {
  match arguments {
    [] => Ok(None),
    [flag, count] if flag == "--frames" => Ok(Some(count.parse()?)),
    _ => Err(Error::Usage),
  }
}

impl ApplicationHandler for Client {
  fn resumed(&mut self, events: &ActiveEventLoop) {
    let Some(font) = self.font.take() else { return };
    match Open::new(events, font) {
      Ok(open) => {
        open.window.request_redraw();
        self.open = Some(open);
      }
      Err(fault) => {
        self.fault = Some(fault);
        events.exit();
      }
    }
  }

  fn window_event(&mut self, events: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
    let Some(open) = self.open.as_mut() else { return };
    match event {
      WindowEvent::CloseRequested => events.exit(),
      WindowEvent::Resized(size) => open.resize(size),
      WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
        if open.view.key(&event.logical_key) {
          open.window.request_redraw();
        }
      }
      WindowEvent::RedrawRequested => {
        open.draw();
        if let Some(left) = self.frames.as_mut() {
          *left = left.saturating_sub(1);
          if *left == 0 {
            events.exit();
          } else {
            open.window.request_redraw();
          }
        }
      }
      _ => {}
    }
  }
}

impl Open {
  fn new(events: &ActiveEventLoop, font: SystemFont) -> Result<Self, Error> {
    let attributes =
      Window::default_attributes().with_title("arena").with_inner_size(LogicalSize::new(1280, 800));
    let window = Arc::new(events.create_window(attributes)?);
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
      Box::new(events.owned_display_handle()),
    ));
    let surface = instance.create_surface(Arc::clone(&window))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
      compatible_surface: Some(&surface),
      ..wgpu::RequestAdapterOptions::default()
    }))?;
    let (device, queue) =
      pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
    let size = window.inner_size();
    let mut config = surface
      .get_default_config(&adapter, size.width.max(1), size.height.max(1))
      .ok_or(Error::Format)?;
    let format = config.format.remove_srgb_suffix();
    config.view_formats = vec![format];
    surface.configure(&device, &config);
    let screen = Screen::new(Painter::new(device.clone(), queue.clone(), format, font), FIRST);
    let view = View::new(&FIRST, [config.width, config.height]);
    Ok(Self { window, surface, device, queue, config, format, screen, view })
  }

  fn resize(&mut self, size: PhysicalSize<u32>) {
    if size.width == 0 || size.height == 0 {
      return;
    }
    self.config.width = size.width;
    self.config.height = size.height;
    self.surface.configure(&self.device, &self.config);
    self.view.resize([size.width, size.height]);
    self.window.request_redraw();
  }

  fn draw(&mut self) {
    let frame = match self.surface.get_current_texture() {
      wgpu::CurrentSurfaceTexture::Success(frame)
      | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
      wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
      _ => {
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
        return;
      }
    };
    let target = frame.texture.create_view(&wgpu::TextureViewDescriptor {
      format: Some(self.format),
      ..wgpu::TextureViewDescriptor::default()
    });
    self.screen.draw(&self.view, &target);
    self.window.pre_present_notify();
    self.queue.present(frame);
  }
}
