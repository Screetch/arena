struct Spot {
  @builtin(position) at: vec4f,
  @location(0) @interpolate(flat) origin: vec2f,
}

@group(0) @binding(0) var<uniform> screen: vec4f;
@group(1) @binding(0) var tile: texture_2d<f32>;

@vertex
fn place(@builtin(vertex_index) corner: u32, @location(0) origin: vec2i) -> Spot {
  var corners = array(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0), vec2f(1.0, 1.0));
  let side = vec2f(textureDimensions(tile));
  let at = (vec2f(origin) + corners[corner] * side) / screen.xy;
  return Spot(vec4f(at.x * 2.0 - 1.0, 1.0 - at.y * 2.0, 0.0, 1.0), vec2f(origin));
}

@fragment
fn show(spot: Spot) -> @location(0) vec4f {
  return textureLoad(tile, vec2i(spot.at.xy - spot.origin), 0);
}
