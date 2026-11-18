//! The graphics adapter of the tests, which draw to a texture and never to a window.

use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use crate::font::Glyphs;
use crate::paint::Painter;

/// Covers the left half of each cell, so that a pixel there shows the ink, and a pixel of the
/// right half the fill.
pub(crate) struct Halves;

impl Glyphs for Halves {
  fn draw(&self, _: char, side: u32) -> Vec<u8> {
    (0..side * side).map(|at| if at % side < side / 2 { 255 } else { 0 }).collect()
  }
}

pub(crate) struct Gpu {
  device: wgpu::Device,
  queue: wgpu::Queue,
}

static GPU: LazyLock<Mutex<Gpu>> = LazyLock::new(|| Mutex::new(Gpu::new()));

impl Gpu {
  /// The one device of the test binary, which one test holds at a time. The harness runs the tests
  /// on parallel threads, and devices that draw at the same time in one process crash it on the
  /// adapter of a runner of Windows.
  pub(crate) fn held() -> MutexGuard<'static, Self> {
    GPU.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn new() -> Self {
    let instance =
      wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
      pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("the runner has a graphics adapter");
    let (device, queue) =
      pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("the adapter gives a device");
    Self { device, queue }
  }

  pub(crate) fn painter(&self) -> Painter<Halves> {
    Painter::new(self.device.clone(), self.queue.clone(), wgpu::TextureFormat::Rgba8Unorm, Halves)
  }

  /// Lets `draw` draw on a texture of `size` pixels, and reads its pixels back, row by row.
  pub(crate) fn painted(
    &self,
    size: [u32; 2],
    draw: impl FnOnce(&wgpu::TextureView),
  ) -> Vec<[u8; 4]> {
    let [width, height] = size;
    let texture = self.device.create_texture(&wgpu::TextureDescriptor {
      label: None,
      size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: wgpu::TextureFormat::Rgba8Unorm,
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
      view_formats: &[],
    });
    draw(&texture.create_view(&wgpu::TextureViewDescriptor::default()));
    let row = (width * 4).next_multiple_of(256);
    let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
      label: None,
      size: u64::from(row * height),
      usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
      mapped_at_creation: false,
    });
    let mut encoder =
      self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
      texture.as_image_copy(),
      wgpu::TexelCopyBufferInfo {
        buffer: &buffer,
        layout: wgpu::TexelCopyBufferLayout {
          offset: 0,
          bytes_per_row: Some(row),
          rows_per_image: None,
        },
      },
      texture.size(),
    );
    self.queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |mapped| mapped.expect("the buffer maps"));
    self
      .device
      .poll(wgpu::PollType::Wait { submission_index: None, timeout: None })
      .expect("the copy ends");
    let bytes = buffer.slice(..).get_mapped_range().expect("the buffer is mapped");
    bytes
      .chunks(row as usize)
      .flat_map(|line| line[..width as usize * 4].chunks(4).map(|pixel| pixel.try_into().unwrap()))
      .collect()
  }
}
