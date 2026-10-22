@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var filtering: sampler;
struct Feedback { effect:vec4f, shift:vec4f, clock:vec4f, output:vec4f }
@group(1) @binding(0) var<uniform> f:Feedback;
struct V { @builtin(position) p:vec4f,@location(0) uv:vec2f }
@vertex fn vertex(@builtin(vertex_index) i:u32)->V {
    let p=array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3))[i];
    var o:V;o.p=vec4f(p,0,1);o.uv=p*vec2f(0.5,-0.5)+0.5;return o;
}
fn sample_at(uv:vec2f)->vec4f {
    let pixel=textureSample(image,filtering,uv);
    return pixel*select(0.0,1.0,all(uv>=vec2f(0)) && all(uv<=vec2f(1)));
}
@fragment fn fragment(v:V)->@location(0) vec4f {
    let dt=f.clock.x;let angle=-f.effect.z*dt;
    let aspect=f.output.x/f.output.y;
    let center=(v.uv-0.5-f.shift.xy*dt)*vec2f(aspect,1.0)*exp(-f.effect.y*dt);
    var uv=vec2f(center.x*cos(angle)-center.y*sin(angle),center.x*sin(angle)+center.y*cos(angle))/vec2f(aspect,1.0)+0.5;
    uv+=vec2f(sin(uv.y*13.0+f.clock.y*0.3),cos(uv.x*11.0-f.clock.y*0.2))*f.effect.w*(0.2+f.clock.z)*dt*0.03;
    let split=vec2f(f.shift.z*dt*45.0/f.output.x,0.0);
    let a=sample_at(uv);let r=sample_at(uv+split);let b=sample_at(uv-split);
    return vec4f(r.r,a.g,b.b,max(a.a,max(r.a,b.a)))*exp(-dt/max(f.effect.x,0.001));
}
