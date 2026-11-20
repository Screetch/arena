//! What differs in a browser, run in a headless browser by `sh script/web.sh test`: the glyphs
//! that the 2D text of a canvas draws, and the drawing of the client on the WebGL2 of the page. The
//! other tests of the client build only for a system, so these stand apart from their modules.
#![cfg(target_arch = "wasm32")]
#![expect(
  clippy::expect_used,
  reason = "a test fails at once when the page gives no canvas or no graphics adapter"
)]

use std::cell::Cell;
use std::rc::Rc;

use arena_client::cells::{Glyph, RAMP};
use arena_client::font::{CanvasFont, Glyphs};
use arena_client::paint::{Painter, Plan};
use arena_client::play::Play;
use arena_client::screen::Screen;
use arena_sim::map::FIRST;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[wasm_bindgen_test]
fn the_canvas_of_the_browser_draws_each_character_of_the_ramp_but_the_space() {
  let font = CanvasFont::new().expect("the page gives a canvas with a 2D context");
  let ink = |character: char, side: u32| {
    let mask = font.draw(character, [side, side]);
    assert_eq!(mask.len(), (side * side) as usize, "{character}");
    mask.iter().map(|&coverage| u32::from(coverage)).sum::<u32>()
  };
  for (level, &character) in RAMP.iter().enumerate() {
    assert_eq!(ink(character, 16) == 0, level == 0, "{character}");
  }
  assert!(ink('.', 16) < ink('@', 16));
  assert!(ink('@', 8) > 0);
}

#[wasm_bindgen_test]
fn a_glyph_leaves_no_ink_on_the_next_one() {
  let font = CanvasFont::new().expect("the page gives a canvas with a 2D context");
  assert_ne!(font.draw('@', [16, 16]), vec![0; 256]);
  assert_eq!(font.draw(' ', [16, 16]), vec![0; 256]);
  assert_eq!(font.draw('@', [3, 4]).len(), 12);
}

#[wasm_bindgen_test]
async fn the_webgl2_of_the_page_draws_the_figure_over_the_map_and_one_frame_the_same_twice() {
  let (device, queue) = webgl().await;
  let font = CanvasFont::new().expect("the page gives a canvas with a 2D context");
  let mut screen = Screen::new(Painter::new(device.clone(), queue.clone(), FORMAT, font), FIRST);
  let mut play = Play::new([320, 200]);
  let sprites = play.frame(0.0);
  assert_eq!(sprites.len(), 1);
  let view = *play.view();
  let map = painted(&device, &queue, [320, 200], |target| screen.draw(&view, &[], target)).await;
  let figure =
    painted(&device, &queue, [320, 200], |target| screen.draw(&view, &sprites, target)).await;
  let again =
    painted(&device, &queue, [320, 200], |target| screen.draw(&view, &sprites, target)).await;
  assert_ne!(map, figure);
  assert_eq!(figure, again);
}

#[wasm_bindgen_test]
async fn the_webgl2_of_the_page_blurs_the_glow_of_a_patch_about_its_cells() {
  let (device, queue) = webgl().await;
  let font = CanvasFont::new().expect("the page gives a canvas with a 2D context");
  let mut painter = Painter::new(device.clone(), queue.clone(), FORMAT, font);
  let filled = Glyph {
    column: 0,
    row: 0,
    level: 1,
    ink: [0; 3],
    fill: [200, 100, 50],
    opacity: 0.0,
    fill_opacity: 1.0,
  };
  let tile = painter.bake(Plan { cells: [1, 1], cell: [16, 16], margin: 12 }, &[filled]);
  let glow = painter.glow(&tile, 4.0, [1.0, 0.0, 0.0, 1.0]);
  let tiles = [(&glow, [0, 0]), (&tile, [0, 0])];
  let pixels =
    painted(&device, &queue, [40, 40], |target| painter.paint(target, [40, 40], &tiles)).await;
  let at = |x: u32, y: u32| pixels[(y * 40 + x) as usize];
  assert_eq!(at(20, 20), [200, 100, 50, 255]);
  let near = at(9, 20);
  assert!(near[0] > 43 && near[1] < 47 && near[2] < 58, "{near:?}");
}

/// The device of the WebGL2 of the page, which a browser with no WebGPU draws with. The headless
/// browser of a runner gives no WebGPU.
async fn webgl() -> (wgpu::Device, wgpu::Queue) {
  let mut described = wgpu::InstanceDescriptor::new_without_display_handle();
  described.backends = wgpu::Backends::GL;
  let instance = wgpu::Instance::new(described);
  let canvas = web_sys::window()
    .and_then(|window| window.document())
    .and_then(|document| document.create_element("canvas").ok())
    .and_then(|element| element.dyn_into::<web_sys::HtmlCanvasElement>().ok())
    .expect("the page gives a canvas");
  let surface = instance
    .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
    .expect("the canvas gives a surface of WebGL2");
  let adapter = instance
    .request_adapter(&wgpu::RequestAdapterOptions {
      compatible_surface: Some(&surface),
      ..wgpu::RequestAdapterOptions::default()
    })
    .await
    .expect("the page gives the adapter of WebGL2");
  let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
  adapter
    .request_device(&wgpu::DeviceDescriptor {
      required_limits: limits,
      ..wgpu::DeviceDescriptor::default()
    })
    .await
    .expect("the adapter gives a device")
}

/// Lets `draw` draw on a texture of `size` pixels, and reads its pixels back, row by row.
async fn painted(
  device: &wgpu::Device,
  queue: &wgpu::Queue,
  size: [u32; 2],
  draw: impl FnOnce(&wgpu::TextureView),
) -> Vec<[u8; 4]> {
  let [width, height] = size;
  let texture = device.create_texture(&wgpu::TextureDescriptor {
    label: None,
    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: FORMAT,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    view_formats: &[],
  });
  draw(&texture.create_view(&wgpu::TextureViewDescriptor::default()));
  let row = (width * 4).next_multiple_of(256);
  let buffer = device.create_buffer(&wgpu::BufferDescriptor {
    label: None,
    size: u64::from(row * height),
    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
  });
  let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
  let layout =
    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: None };
  encoder.copy_texture_to_buffer(
    texture.as_image_copy(),
    wgpu::TexelCopyBufferInfo { buffer: &buffer, layout },
    texture.size(),
  );
  queue.submit([encoder.finish()]);
  let mapped = Rc::new(Cell::new(None));
  let told = Rc::clone(&mapped);
  buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| told.set(Some(result)));
  let result = loop {
    device.poll(wgpu::PollType::Poll).expect("the device polls");
    if let Some(result) = mapped.take() {
      break result;
    }
    later().await;
  };
  result.expect("the buffer maps");
  let bytes = buffer.slice(..).get_mapped_range().expect("the buffer is mapped");
  bytes
    .chunks(row as usize)
    .flat_map(|line| {
      line[..width as usize * 4]
        .chunks(4)
        .map(|pixel| pixel.try_into().expect("a pixel is 4 bytes"))
    })
    .collect()
}

/// Waits for the next task of the page. WebGL2 tells that the GPU ended a copy only between two
/// tasks of the page, so a wait in one task never ends.
async fn later() {
  let tick = js_sys::Promise::new(&mut |done, _| {
    web_sys::window()
      .expect("the page has a window")
      .set_timeout_with_callback(&done)
      .expect("the page sets a timeout");
  });
  JsFuture::from(tick).await.expect("a timeout of the page resolves");
}
