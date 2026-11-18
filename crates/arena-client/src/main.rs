//! The window of the client: it moves the events of the window to the view, and draws each frame.
//!
//! On a system, `arena-client --frames <n>` ends after n frames, as the gate does under a virtual
//! display. In a browser, the window is a canvas that the page sizes, and wgpu draws with WebGPU,
//! else with WebGL2.

use std::sync::Arc;

use arena_client::font;
use arena_client::paint::Painter;
use arena_client::screen::Screen;
use arena_client::view::View;
use arena_sim::map::FIRST;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy, OwnedDisplayHandle};
use winit::window::{Window, WindowAttributes, WindowId};

#[cfg(not(target_arch = "wasm32"))]
type Font = font::SystemFont;
#[cfg(target_arch = "wasm32")]
type Font = font::CanvasFont;

/// The graphics of the window, or why they cannot draw.
type Opened = Result<Open, Error>;

#[derive(Debug, thiserror::Error)]
enum Error {
  #[cfg(not(target_arch = "wasm32"))]
  #[error("the client takes no argument, or --frames <n>")]
  Usage,
  #[cfg(not(target_arch = "wasm32"))]
  #[error("--frames takes a whole number of frames")]
  Frames(#[from] std::num::ParseIntError),
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
  font: Option<Font>,
  open: Option<Open>,
  proxy: EventLoopProxy<Opened>,
  fault: Option<Error>,
}

struct Open {
  window: Arc<Window>,
  surface: wgpu::Surface<'static>,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  screen: Screen<Font>,
  view: View,
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Error> {
  let frames = frames(&std::env::args().skip(1).collect::<Vec<_>>())?;
  let font = Font::find()?;
  eprintln!("arena draws its characters with the font {}", font.name());
  let events = EventLoop::with_user_event().build()?;
  let mut client = Client::new(&events, frames, font);
  events.run_app(&mut client)?;
  client.fault.map_or(Ok(()), Err)
}

#[cfg(not(target_arch = "wasm32"))]
fn frames(arguments: &[String]) -> Result<Option<u32>, Error> {
  match arguments {
    [] => Ok(None),
    [flag, count] if flag == "--frames" => Ok(Some(count.parse()?)),
    _ => Err(Error::Usage),
  }
}

#[cfg(target_arch = "wasm32")]
fn main() {
  use winit::platform::web::EventLoopExtWebSys;
  let started = Font::new().map_err(Error::from).and_then(|font| {
    let events = EventLoop::with_user_event().build()?;
    Ok((events, font))
  });
  match started {
    Ok((events, font)) => {
      let client = Client::new(&events, None, font);
      events.spawn_app(client);
    }
    Err(fault) => report(&fault),
  }
}

impl Client {
  fn new(events: &EventLoop<Opened>, frames: Option<u32>, font: Font) -> Self {
    Self { frames, font: Some(font), open: None, proxy: events.create_proxy(), fault: None }
  }

  fn fail(&mut self, events: &ActiveEventLoop, fault: Error) {
    self.fault = Some(fault);
    events.exit();
  }
}

impl ApplicationHandler<Opened> for Client {
  fn resumed(&mut self, events: &ActiveEventLoop) {
    let Some(font) = self.font.take() else { return };
    match events.create_window(attributes()) {
      Ok(window) => {
        open(Open::new(Arc::new(window), events.owned_display_handle(), font), &self.proxy);
      }
      Err(fault) => self.fail(events, fault.into()),
    }
  }

  fn user_event(&mut self, events: &ActiveEventLoop, opened: Opened) {
    match opened {
      Ok(open) => {
        open.window.request_redraw();
        self.open = Some(open);
      }
      Err(fault) => self.fail(events, fault),
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

  #[cfg(target_arch = "wasm32")]
  fn exiting(&mut self, _: &ActiveEventLoop) {
    if let Some(fault) = &self.fault {
      report(fault);
    }
  }
}

#[cfg(not(target_arch = "wasm32"))]
fn attributes() -> WindowAttributes {
  let size = winit::dpi::LogicalSize::new(1280, 800);
  Window::default_attributes().with_title("arena").with_inner_size(size)
}

#[cfg(target_arch = "wasm32")]
fn attributes() -> WindowAttributes {
  use winit::platform::web::WindowAttributesExtWebSys;
  Window::default_attributes().with_append(true)
}

/// Opens the graphics of the window at once on a system. The loop gives them to the client as an
/// event, as in a browser.
#[cfg(not(target_arch = "wasm32"))]
fn open(opening: impl Future<Output = Opened>, proxy: &EventLoopProxy<Opened>) {
  give(proxy, pollster::block_on(opening));
}

/// Opens the graphics of the window on the loop of the page, since a browser gives its graphics
/// adapter only to a future.
#[cfg(target_arch = "wasm32")]
fn open(opening: impl Future<Output = Opened> + 'static, proxy: &EventLoopProxy<Opened>) {
  let proxy = proxy.clone();
  wasm_bindgen_futures::spawn_local(async move { give(&proxy, opening.await) });
}

fn give(proxy: &EventLoopProxy<Opened>, opened: Opened) {
  // The loop closes only when the client ends, and then no window waits for its graphics.
  proxy.send_event(opened).unwrap_or_default();
}

/// Shows on the page why the client cannot run, in place of its canvas.
#[cfg(target_arch = "wasm32")]
fn report(fault: &Error) {
  let body = web_sys::window().and_then(|window| window.document()).and_then(|page| page.body());
  if let Some(body) = body {
    body.set_text_content(Some(&fault.to_string()));
  }
}

impl Open {
  async fn new(window: Arc<Window>, display: OwnedDisplayHandle, font: Font) -> Opened {
    let instance = wgpu::util::new_instance_with_webgpu_detection(
      wgpu::InstanceDescriptor::new_with_display_handle(Box::new(display)),
    )
    .await;
    let surface = instance.create_surface(Arc::clone(&window))?;
    let adapter = instance
      .request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: Some(&surface),
        ..wgpu::RequestAdapterOptions::default()
      })
      .await?;
    let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
    let (device, queue) = adapter
      .request_device(&wgpu::DeviceDescriptor {
        required_limits: limits,
        ..wgpu::DeviceDescriptor::default()
      })
      .await?;
    let size = window.inner_size();
    let mut config = surface
      .get_default_config(&adapter, size.width.max(1), size.height.max(1))
      .ok_or(Error::Format)?;
    config.format = config.format.remove_srgb_suffix();
    if !surface.get_capabilities(&adapter).formats.contains(&config.format) {
      return Err(Error::Format);
    }
    surface.configure(&device, &config);
    let screen =
      Screen::new(Painter::new(device.clone(), queue.clone(), config.format, font), FIRST);
    let view = View::new(&FIRST, [config.width, config.height]);
    window.focus_window();
    Ok(Self { window, surface, device, queue, config, screen, view })
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
    let target = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
    self.screen.draw(&self.view, &target);
    self.window.pre_present_notify();
    self.queue.present(frame);
  }
}
