struct Frame {
    clock: vec4f, flock: vec4f, forces: vec4f, orbs: vec4f,
    look: vec4f, light: vec4f, view: vec4f,
    axis_x: vec4f, axis_y: vec4f, axis_z: vec4f,
    audio: vec4f, output: vec4f,
    particle_style: vec4f, particle_detail: vec4f, particle_color: vec4f, particle_light: vec4f, color_start: vec4f, color_end: vec4f,
    orb_style:vec4f, orb_shape:vec4f, orb_material:vec4f, orb_surface:vec4f, orb_light:vec4f, orb_color:vec4f, orb_tint:vec4f, orb_start:vec4f, orb_end:vec4f, camera:vec4f, behavior:vec4f, dynamics:vec4f, field:vec4f, circulation:vec4f, paths:vec4f, path_style:vec4f, path_clock:vec4f, orb_field:vec4f, orb_barrier:vec4f, orb_optics:vec4f, orb_corona:vec4f, orb_flare:vec4f, orb_colors:array<vec4f,5>,
}
struct Particle {position:vec4f,velocity:vec4f}
@group(0) @binding(0) var<uniform> f:Frame;
@group(0) @binding(1) var<storage,read> points:array<vec4f>;
@group(0) @binding(2) var<storage,read> particles:array<Particle>;
fn hash(n: u32) -> f32 {
    var x = n * 747796405u + 2891336453u;
    x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;
    return f32((x >> 22u) ^ x) / 4294967295.0;
}
fn safe(v: vec3f) -> vec3f { return v / max(length(v), 0.0001); }
fn orb(i: u32) -> vec3f {
    let a = f32(i) * 6.2831853 / max(f.orbs.x, 1.0) + f.clock.y * f.orbs.w;
    return vec3f(cos(a), sin(a * 1.3) * 0.35, sin(a)) * f.orbs.y;
}
fn camera(p: vec3f) -> vec3f { return f.axis_x.xyz * p.x + f.axis_y.xyz * p.y + f.axis_z.xyz * p.z; }
fn project(p: vec3f) -> vec2f {
    let scale=select(f.camera.y/max(f.camera.x+p.z,0.3),2.0/f.camera.x,f.camera.z>0.5);
    return p.xy*f.view.y*min(f.view.x,1.0)*scale/vec2f(f.view.x,1.0)+f.view.zw;
}
fn palette(t: f32, id:f32, start:vec3f, end:vec3f) -> vec3f {
    let u = fract(t);
    // Non-spectral palettes ping-pong, avoiding a hard jump when animated.
    let k = 1.0 - abs(fract(t * 0.5) * 2.0 - 1.0);
    var a = vec3f(0.0); var b = vec3f(1.0); var c = vec3f(1.0);
    switch u32(id) {
        case 0u: { return 0.55 + 0.45 * cos(6.2831853 * (vec3f(0.05,0.32,0.60) + u)); }
        case 1u: { a=vec3f(0.3,0.002,0.008); b=vec3f(1.0,0.08,0.003); c=vec3f(1.0,0.8,0.12); }
        case 2u: { a=vec3f(0.008,0.035,0.35); b=vec3f(0.0,0.6,1.0); c=vec3f(0.7,1.0,1.0); }
        case 3u: { a=vec3f(0.2,0.005,0.6); b=vec3f(0.0,0.65,0.35); c=vec3f(0.4,1.0,0.05); }
        case 4u: { a=vec3f(0.05,0.3,1.0); b=vec3f(0.8,0.01,0.8); c=vec3f(1.0,0.16,0.28); }
        case 5u: { a=vec3f(0.3,0.045,0.005); b=vec3f(1.0,0.45,0.02); c=vec3f(1.0,0.9,0.45); }
        case 6u: { return vec3f(1.0); }
        default: { return mix(pow(start,vec3f(2.2)),pow(end,vec3f(2.2)),k); }
    }
    return mix(mix(a,b,clamp(k*2.0,0.0,1.0)),c,clamp(k*2.0-1.0,0.0,1.0));
}
fn color_driver(world:vec3f, velocity:vec3f, identity:f32)->f32 {
    switch u32(f.particle_color.y) {
        case 1u: { return clamp(length(velocity)/max(f.flock.w,0.01),0.0,1.0); }
        case 2u: { return clamp(length(world)/f.audio.w,0.0,1.0); }
        case 3u: { return clamp(world.y/f.audio.w*0.5+0.5,0.0,1.0); }
        case 4u: {
            var nearest=0u; var distance=1e10;
            for(var j=0u; j<u32(f.orbs.x); j++) { let d=length(world-orb(j)); if d<distance {distance=d;nearest=j;} }
            return f32(nearest)/max(f.orbs.x-1.0,1.0);
        }
        default: { return identity; }
    }
}
struct Vertex { @builtin(position) clip:vec4f, @location(0) color:vec3f, @location(1) edge:f32, @location(2) camera_point:vec3f }
@vertex fn vertex(@builtin(vertex_index) vi:u32,@builtin(instance_index) ii:u32)->Vertex {
    let samples=u32(f.path_clock.y);
    let segments=max(samples,2u)-1u;
    let tracer=ii/segments; let age=ii%segments;
    let head=u32(f.path_clock.x);
    let start=points[((head+128u-age)%128u)*512u+tracer];
    let end=points[((head+127u-age)%128u)*512u+tracer];
    let p=camera(start.xyz);
    let cutoff=clamp((start.w-(f.clock.y-f.paths.z))/max(start.w-end.w,0.0001),0.0,1.0);
    let q=camera(mix(start.xyz,end.xyz,cutoff));
    let a=project(p);let b=project(q);
    let pixel_direction=(b-a)*f.output.xy;
    let normal=vec2f(-pixel_direction.y,pixel_direction.x)/max(length(pixel_direction),0.0001);
    let corners=array<vec2f,6>(vec2f(0,-1),vec2f(1,-1),vec2f(0,1),vec2f(0,1),vec2f(1,-1),vec2f(1,1));
    let corner=corners[vi];
    var out:Vertex;out.clip=vec4f(mix(a,b,corner.x)+normal*corner.y*f.paths.w/f.output.xy,0.5,1.0);
    if f.clock.y-start.w>f.paths.z || f.camera.x+p.z<0.3 || f.camera.x+q.z<0.3 {out.clip=vec4f(5,5,0.5,1);}
    let particle=particles[min(tracer*u32(f.path_clock.w),u32(f.clock.z)-1u)];
    let driver=color_driver(particle.position.xyz,particle.velocity.xyz,particle.position.w);
    var color=palette(driver*f.particle_color.z+f.look.w+f.clock.y*f.particle_light.z,f.particle_color.x,f.color_start.rgb,f.color_end.rgb);
    let gray=dot(color,vec3f(0.2126,0.7152,0.0722));
    color=max(mix(vec3f(gray),color,f.particle_color.w),vec3f(0));
    let sample_time=mix(start.w,mix(start.w,end.w,cutoff),corner.x);
    let fade=pow(max(1.0-(f.clock.y-sample_time)/f.paths.z,0.001),f.path_style.y);
    let dt=select(f.clock.x,1.0/60.0,f.output.w>0.5 || f.clock.w>0.5);
    let normalize_trail=select(1.0,1.0-exp2(-dt/max(f.light.w,0.001)),f.light.w>0.001);
    out.camera_point=mix(p,q,corner.x);
    out.color=color*optical_illumination(out.camera_point)*f.path_style.x*f.look.z*fade*normalize_trail;out.edge=corner.y;return out;
}
@fragment fn fragment(in:Vertex)->@location(0) vec4f {return vec4f(in.color*optical_visibility(in.camera_point,99u)*(1.0-smoothstep(0.65,1.0,abs(in.edge))),0.0);}
