struct Blur {
  step: vec2f,
  sigma: f32,
  reach: f32,
  colour: vec4f,
}

@group(0) @binding(0) var<uniform> blur: Blur;
@group(0) @binding(1) var source: texture_2d<f32>;

@vertex
fn place(@builtin(vertex_index) corner: u32) -> @builtin(position) vec4f {
  var corners = array(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(-1.0, 1.0), vec2f(1.0, 1.0));
  return vec4f(corners[corner], 0.0, 1.0);
}

@fragment
fn spread(@builtin(position) at: vec4f) -> @location(0) vec4f {
  let size = vec2i(textureDimensions(source));
  let pixel = vec2i(at.xy);
  var sum = 0.0;
  var total = 0.0;
  for (var d = -blur.reach; d <= blur.reach; d += 1.0) {
    let weight = exp(-d * d / (2.0 * blur.sigma * blur.sigma));
    let point = pixel + vec2i(blur.step * d);
    if all(point >= vec2i(0)) && all(point < size) {
      sum += textureLoad(source, point, 0).a * weight;
    }
    total += weight;
  }
  return blur.colour * (sum / total);
}
