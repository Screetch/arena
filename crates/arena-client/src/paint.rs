//! The drawing of the map with wgpu.
//!
//! Each tile of cells is drawn once into a texture of its own, and each frame places on the target
//! the tiles that the view shows. The colours blend in sRGB as they are stored, with no
//! conversion to linear light.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use crate::cells::{BACK, Glyph, Grid, RAMP, TILE};
use crate::font::Glyphs;

/// The glyph of one cell, as the shader of the tiles reads it.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Mark {
  cell: [u16; 2],
  level: u32,
  ink: [u8; 4],
  fill: [u8; 4],
  opacity: f32,
}

/// The pipelines that draw the tiles and place them on a target, with the glyphs of their cells.
pub struct Painter<G> {
  device: wgpu::Device,
  queue: wgpu::Queue,
  glyphs: G,
  bake: wgpu::RenderPipeline,
  show: wgpu::RenderPipeline,
  atlases: HashMap<u32, wgpu::BindGroup>,
  screen: wgpu::Buffer,
  screen_group: wgpu::BindGroup,
}

/// A tile of cells, drawn into a texture of 48 by 48 cells, with its colours premultiplied.
pub struct Tile {
  group: wgpu::BindGroup,
}

impl<G: Glyphs> Painter<G> {
  /// The painter of targets of `format`, which must not be an sRGB format, so that the colours
  /// blend as they are stored.
  pub fn new(
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    glyphs: G,
  ) -> Self {
    let bake =
      pipeline(&device, include_str!("bake.wgsl"), wgpu::TextureFormat::Rgba8Unorm, &MARKS);
    let show = pipeline(&device, include_str!("show.wgsl"), format, &SPOTS);
    let screen = device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("screen"),
      size: 16,
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
    });
    let screen_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("screen"),
      layout: &show.get_bind_group_layout(0),
      entries: &[wgpu::BindGroupEntry { binding: 0, resource: screen.as_entire_binding() }],
    });
    Self { device, queue, glyphs, bake, show, atlases: HashMap::new(), screen, screen_group }
  }

  /// Draws the glyphs of one tile of `grid` into a texture.
  pub fn bake(&mut self, grid: Grid, glyphs: &[Glyph]) -> Tile {
    let cell = grid.cell();
    let side = cell * u32::from(TILE);
    let atlas = self.atlases.entry(cell).or_insert_with(|| {
      atlas(&self.device, &self.queue, &self.glyphs, &self.bake.get_bind_group_layout(0), cell)
    });
    let texture = self.device.create_texture(&wgpu::TextureDescriptor {
      label: Some("tile"),
      size: wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: wgpu::TextureFormat::Rgba8Unorm,
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
      view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder =
      self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
      let attachments = cleared(&view, wgpu::Color::TRANSPARENT);
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &attachments,
        ..wgpu::RenderPassDescriptor::default()
      });
      if !glyphs.is_empty() {
        let marks: Vec<Mark> = glyphs.iter().map(Mark::of).collect();
        let marks = self.device.create_buffer_init(&BufferInitDescriptor {
          label: Some("marks"),
          contents: bytemuck::cast_slice(&marks),
          usage: wgpu::BufferUsages::VERTEX,
        });
        pass.set_pipeline(&self.bake);
        pass.set_bind_group(0, &*atlas, &[]);
        pass.set_vertex_buffer(0, marks.slice(..));
        pass.draw(0..4, 0..u32::try_from(glyphs.len()).unwrap_or(u32::MAX));
      }
    }
    self.queue.submit([encoder.finish()]);
    let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("tile"),
      layout: &self.show.get_bind_group_layout(1),
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::TextureView(&view),
      }],
    });
    Tile { group }
  }

  /// Fills the target of `size` pixels with the colour behind the map, and places each tile with
  /// its top left corner at its origin, in pixels of the target.
  pub fn paint(&self, target: &wgpu::TextureView, size: [u32; 2], tiles: &[(&Tile, [i32; 2])]) {
    let screen = [float(size[0]), float(size[1]), 0.0, 0.0];
    self.queue.write_buffer(&self.screen, 0, bytemuck::cast_slice(&screen));
    let back = BACK.map(|channel| f64::from(channel) / 255.0);
    let back = wgpu::Color { r: back[0], g: back[1], b: back[2], a: 1.0 };
    let mut encoder =
      self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
      let attachments = cleared(target, back);
      let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &attachments,
        ..wgpu::RenderPassDescriptor::default()
      });
      if !tiles.is_empty() {
        let origins: Vec<[i32; 2]> = tiles.iter().map(|&(_, origin)| origin).collect();
        let origins = self.device.create_buffer_init(&BufferInitDescriptor {
          label: Some("origins"),
          contents: bytemuck::cast_slice(&origins),
          usage: wgpu::BufferUsages::VERTEX,
        });
        pass.set_pipeline(&self.show);
        pass.set_bind_group(0, &self.screen_group, &[]);
        pass.set_vertex_buffer(0, origins.slice(..));
        for (&(tile, _), index) in tiles.iter().zip(0u32..) {
          pass.set_bind_group(1, &tile.group, &[]);
          pass.draw(0..4, index..index + 1);
        }
      }
    }
    self.queue.submit([encoder.finish()]);
  }
}

impl Mark {
  fn of(glyph: &Glyph) -> Self {
    let [r, g, b] = glyph.ink;
    let [fr, fg, fb] = glyph.fill;
    Self {
      cell: [glyph.column, glyph.row],
      level: u32::from(glyph.level),
      ink: [r, g, b, 255],
      fill: [fr, fg, fb, 255],
      opacity: glyph.opacity,
    }
  }
}

const MARKS: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
  array_stride: size_of::<Mark>() as u64,
  step_mode: wgpu::VertexStepMode::Instance,
  attributes: &wgpu::vertex_attr_array![
    0 => Uint16x2, 1 => Uint32, 2 => Unorm8x4, 3 => Unorm8x4, 4 => Float32
  ],
};

const SPOTS: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
  array_stride: size_of::<[i32; 2]>() as u64,
  step_mode: wgpu::VertexStepMode::Instance,
  attributes: &wgpu::vertex_attr_array![0 => Sint32x2],
};

fn pipeline(
  device: &wgpu::Device,
  source: &str,
  format: wgpu::TextureFormat,
  marks: &wgpu::VertexBufferLayout<'_>,
) -> wgpu::RenderPipeline {
  let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: None,
    source: wgpu::ShaderSource::Wgsl(source.into()),
  });
  device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    label: None,
    layout: None,
    vertex: wgpu::VertexState {
      module: &module,
      entry_point: None,
      compilation_options: wgpu::PipelineCompilationOptions::default(),
      buffers: &[Some(marks.clone())],
    },
    primitive: wgpu::PrimitiveState {
      topology: wgpu::PrimitiveTopology::TriangleStrip,
      ..wgpu::PrimitiveState::default()
    },
    depth_stencil: None,
    multisample: wgpu::MultisampleState::default(),
    fragment: Some(wgpu::FragmentState {
      module: &module,
      entry_point: None,
      compilation_options: wgpu::PipelineCompilationOptions::default(),
      targets: &[Some(wgpu::ColorTargetState {
        format,
        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
      })],
    }),
    multiview_mask: None,
    cache: None,
  })
}

fn cleared(
  view: &wgpu::TextureView,
  clear: wgpu::Color,
) -> [Option<wgpu::RenderPassColorAttachment<'_>>; 1] {
  [Some(wgpu::RenderPassColorAttachment {
    view,
    depth_slice: None,
    resolve_target: None,
    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
  })]
}

/// The glyph of each character of the ramp side by side, each in a square of one cell, with the
/// sizes that the shader of the tiles reads.
fn atlas(
  device: &wgpu::Device,
  queue: &wgpu::Queue,
  glyphs: &impl Glyphs,
  layout: &wgpu::BindGroupLayout,
  cell: u32,
) -> wgpu::BindGroup {
  let slots = u32::from(u8::try_from(RAMP.len()).unwrap_or(u8::MAX));
  let width = slots * cell;
  let masks: Vec<Vec<u8>> = RAMP.iter().map(|&character| glyphs.draw(character, cell)).collect();
  let mut bytes = Vec::with_capacity((width * cell) as usize);
  for row in 0..cell as usize {
    for mask in &masks {
      bytes.extend_from_slice(&mask[row * cell as usize..(row + 1) * cell as usize]);
    }
  }
  let texture = device.create_texture_with_data(
    queue,
    &wgpu::TextureDescriptor {
      label: Some("atlas"),
      size: wgpu::Extent3d { width, height: cell, depth_or_array_layers: 1 },
      mip_level_count: 1,
      sample_count: 1,
      dimension: wgpu::TextureDimension::D2,
      format: wgpu::TextureFormat::R8Unorm,
      usage: wgpu::TextureUsages::TEXTURE_BINDING,
      view_formats: &[],
    },
    wgpu::util::TextureDataOrder::LayerMajor,
    &bytes,
  );
  let sizes = device.create_buffer_init(&BufferInitDescriptor {
    label: Some("sizes"),
    contents: bytemuck::cast_slice(&[float(cell), float(cell * u32::from(TILE)), 0.0, 0.0]),
    usage: wgpu::BufferUsages::UNIFORM,
  });
  let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
  device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: Some("atlas"),
    layout,
    entries: &[
      wgpu::BindGroupEntry { binding: 0, resource: sizes.as_entire_binding() },
      wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
    ],
  })
}

#[expect(
  clippy::cast_precision_loss,
  reason = "a size in pixels is far under 2^24, which an f32 holds whole"
)]
fn float(pixels: u32) -> f32 {
  pixels as f32
}

#[cfg(test)]
mod tests {
  use arena_sim::map::FIRST;

  use crate::cells::Grid;
  use crate::gpu::Gpu;

  #[test]
  fn each_cell_shows_its_ink_over_its_fill_at_its_opacity_over_the_back() {
    let gpu = Gpu::new();
    let mut painter = gpu.painter();
    let grid = Grid::new(2.0);
    let tile = painter.bake(grid, &grid.tile(&FIRST, 0, 2));
    let pixels =
      gpu.painted([768, 768], |target| painter.paint(target, [768, 768], &[(&tile, [0, 0])]));
    let at = |x: u32, y: u32| pixels[(y * 768 + x) as usize];
    assert_eq!(at(16 + 4, 160 + 8), [126, 138, 167, 255]);
    assert_eq!(at(16 + 12, 160 + 8), [75, 83, 100, 255]);
    assert_eq!(at(160, 160), [43, 47, 58, 255]);
    assert_eq!(at(37 * 16 + 4, 47 * 16 + 8), [78, 86, 106, 255]);
    assert_eq!(at(37 * 16 + 12, 47 * 16 + 8), [55, 60, 75, 255]);
  }

  #[test]
  fn a_tile_stands_at_its_origin_on_the_target() {
    let gpu = Gpu::new();
    let mut painter = gpu.painter();
    let grid = Grid::new(1.0);
    let tile = painter.bake(grid, &grid.tile(&FIRST, 0, 2));
    let pixels =
      gpu.painted([64, 64], |target| painter.paint(target, [64, 64], &[(&tile, [-8, 16])]));
    let at = |x: u32, y: u32| pixels[(y * 64 + x) as usize];
    assert_eq!(at(1, 18), [126, 138, 167, 255]);
    assert_eq!(at(5, 18), [75, 83, 100, 255]);
    assert_eq!(at(1, 10), [43, 47, 58, 255]);
    assert_eq!(at(40, 18), [43, 47, 58, 255]);
  }

  #[test]
  fn a_tile_with_no_glyph_and_a_view_with_no_tile_show_only_the_back() {
    let gpu = Gpu::new();
    let mut painter = gpu.painter();
    let grid = Grid::new(1.0);
    let glyphs = grid.tile(&FIRST, 2, 0);
    assert_eq!(glyphs, []);
    let empty = painter.bake(grid, &glyphs);
    let back = vec![[43, 47, 58, 255]; 16 * 16];
    assert_eq!(
      gpu.painted([16, 16], |target| painter.paint(target, [16, 16], &[(&empty, [0, 0])])),
      back
    );
    assert_eq!(gpu.painted([16, 16], |target| painter.paint(target, [16, 16], &[])), back);
  }
}
