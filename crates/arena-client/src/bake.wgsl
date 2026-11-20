struct Sizes {
  cell: vec2f,
  texture: vec2f,
  margin: vec2f,
}

struct Mark {
  @builtin(position) at: vec4f,
  @location(0) @interpolate(flat) origin: vec2f,
  @location(1) @interpolate(flat) level: u32,
  @location(2) @interpolate(flat) ink: vec4f,
  @location(3) @interpolate(flat) fill: vec4f,
  @location(4) @interpolate(flat) opacity: f32,
  @location(5) @interpolate(flat) fill_opacity: f32,
}

@group(0) @binding(0) var<uniform> sizes: Sizes;
@group(0) @binding(1) var atlas: texture_2d<f32>;

@vertex
fn place(
  @builtin(vertex_index) corner: u32,
  @location(0) cell: vec2u,
  @location(1) level: u32,
  @location(2) ink: vec4f,
  @location(3) fill: vec4f,
  @location(4) opacity: f32,
  @location(5) fill_opacity: f32,
) -> Mark {
  var corners = array(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0), vec2f(1.0, 1.0));
  let origin = sizes.margin + vec2f(cell) * sizes.cell;
  let at = (origin + corners[corner] * sizes.cell) / sizes.texture;
  let position = vec4f(at.x * 2.0 - 1.0, 1.0 - at.y * 2.0, 0.0, 1.0);
  return Mark(position, origin, level, ink, fill, opacity, fill_opacity);
}

@fragment
fn cover(mark: Mark) -> @location(0) vec4f {
  let pixel = vec2u(mark.at.xy - mark.origin);
  let slot = mark.level * u32(sizes.cell.x);
  let ink = textureLoad(atlas, vec2u(slot + pixel.x, pixel.y), 0).r * mark.opacity;
  let fill = mark.fill_opacity * (1.0 - ink);
  return vec4f(mark.ink.rgb * ink + mark.fill.rgb * fill, ink + fill);
}
