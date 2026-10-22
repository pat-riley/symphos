@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var filtering: sampler;
struct V { @builtin(position) p:vec4f,@location(0) uv:vec2f }
@vertex fn vertex(@builtin(vertex_index) i:u32)->V {let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[i];var o:V;o.p=vec4f(p,0,1);o.uv=p*vec2f(0.5,-0.5)+0.5;return o;}
@fragment fn fragment(v:V)->@location(0) vec4f {return textureSample(image,filtering,v.uv);}
