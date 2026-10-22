@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var filtering: sampler;
struct Feedback { effect:vec4f, shift:vec4f, clock:vec4f, output:vec4f }
@group(1) @binding(0) var<uniform> f:Feedback;
struct V { @builtin(position) p:vec4f,@location(0) uv:vec2f }
@vertex fn vertex(@builtin(vertex_index) i:u32)->V {let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[i];var o:V;o.p=vec4f(p,0,1);o.uv=p*vec2f(0.5,-0.5)+0.5;return o;}
@fragment fn fragment(v:V)->@location(0) vec4f {
    let c=textureSample(image,filtering,v.uv);
    if f.output.z>0.5 {
        // Compress accumulated HDR light while preserving the spectral palette.
        let rgb=c.rgb/(1.0+max(c.r,max(c.g,c.b)));
        return vec4f(rgb,max(c.a,max(rgb.r,max(rgb.g,rgb.b))));
    }
    return c;
}
