@group(0) @binding(0)
var render_target_tex: texture_multisampled_2d<f32>;

@fragment
fn fs_main(in: RenderTargetVertexOutput) -> @location(0) vec4f {
    let mag2 = dot(in.coords, in.coords);
    let mag2_thresh = select(
        1.0,
        3.0e38,
        bool(uniform_params.skip_reprojection),
    );
    if (mag2 >= mag2_thresh) {
        discard;
    }

    let f = uniform_params.f * f32(1 - uniform_params.skip_reprojection);

    let base = (0.5 * (1.0 + mag2)) * f + 1.0 - f;
    let new_coords = in.coords / base;
    let texcoords = 0.5 * vec2f(1.0 + new_coords.x, 1.0 - new_coords.y);

    // TODO sampling inside the shader may be a terrible idea
    let dims = vec2f(textureDimensions(render_target_tex));
    let pixelcoords = texcoords * dims + vec2f(-0.5, -0.5);
    // TODO biased to one side?
    let pixelcoords_low = floor(pixelcoords);
    // TODO is this upper bound necessary?
    let pixelcoords_high = min(pixelcoords_low + vec2f(1.0), dims - vec2f(1.0));
    let pixelcoords_lowhigh = vec2f(pixelcoords_low.x, pixelcoords_high.y);
    let pixelcoords_highlow = vec2f(pixelcoords_high.x, pixelcoords_low.y);

    let pixelcoords_frac = pixelcoords - pixelcoords_low;

    let mag2_low = world_coords_mag2(pixelcoords_low, dims);
    let mag2_lowhigh = world_coords_mag2(pixelcoords_lowhigh, dims);
    let mag2_highlow = world_coords_mag2(pixelcoords_highlow, dims);
    let mag2_high = world_coords_mag2(pixelcoords_high, dims);

    let ucoords_low = vec2u(pixelcoords_low);
    let ucoords_lowhigh = vec2u(pixelcoords_lowhigh);
    let ucoords_highlow = vec2u(pixelcoords_highlow);
    let ucoords_high = vec2u(pixelcoords_high);

    let w_low = (1 - pixelcoords_frac.x) * (1 - pixelcoords_frac.y) * f32(mag2_low < mag2_thresh);
    let w_lowhigh = (1 - pixelcoords_frac.x) * pixelcoords_frac.y * f32(mag2_lowhigh < mag2_thresh);
    let w_highlow = pixelcoords_frac.x * (1 - pixelcoords_frac.y) * f32(mag2_highlow < mag2_thresh);
    let w_high = pixelcoords_frac.x * pixelcoords_frac.y * f32(mag2_high < mag2_thresh);

    // TODO weighting subsamples based on their location would be nice, but what's the pattern?
    // https://mynameismjp.wordpress.com/2010/07/07/msaa-sample-pattern-detector/
    var tex_out = vec4f(0);
    let sample_count = i32(textureNumSamples(render_target_tex));
    for (var i: i32 = 0; i < sample_count; i += 1) {
        tex_out = tex_out //+ textureLoad(render_target_tex, ucoords_high, i);
            + w_low * textureLoad(render_target_tex, ucoords_low, i)
            + w_lowhigh * textureLoad(render_target_tex, ucoords_lowhigh, i)
            + w_highlow * textureLoad(render_target_tex, ucoords_highlow, i)
            + w_high * textureLoad(render_target_tex, ucoords_high, i);
    }

    return tex_out / (f32(sample_count) * (w_low + w_lowhigh + w_highlow + w_high));
}