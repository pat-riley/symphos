struct Frame {
    clock: vec4f, flock: vec4f, forces: vec4f, orbs: vec4f,
    look: vec4f, light: vec4f, view: vec4f,
    axis_x: vec4f, axis_y: vec4f, axis_z: vec4f,
    audio: vec4f, output: vec4f,
    particle_style: vec4f, particle_detail: vec4f, particle_color: vec4f, particle_light: vec4f, color_start: vec4f, color_end: vec4f,
    orb_style:vec4f, orb_shape:vec4f, orb_material:vec4f, orb_surface:vec4f, orb_light:vec4f, orb_color:vec4f, orb_tint:vec4f, orb_start:vec4f, orb_end:vec4f, camera:vec4f, behavior:vec4f, dynamics:vec4f, field:vec4f, circulation:vec4f, paths:vec4f, path_style:vec4f, path_clock:vec4f, orb_field:vec4f, orb_barrier:vec4f, orb_optics:vec4f, orb_corona:vec4f, orb_flare:vec4f, orb_colors:array<vec4f,5>,
}
struct Particle { position: vec4f, velocity: vec4f }
struct Cell { count: atomic<i32>, px: atomic<i32>, py: atomic<i32>, pz: atomic<i32>, vx: atomic<i32>, vy: atomic<i32>, vz: atomic<i32>, pad: i32 }
@group(0) @binding(0) var<uniform> f: Frame;
@group(0) @binding(1) var<storage, read> particles: array<Particle>;
@group(0) @binding(2) var<storage, read_write> next: array<Particle>;
@group(0) @binding(3) var<storage, read_write> grid: array<Cell>;
@group(0) @binding(4) var<storage, read_write> path_history: array<vec4f>;
fn hash(n: u32) -> f32 {
    var x = n * 747796405u + 2891336453u;
    x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;
    return f32((x >> 22u) ^ x) / 4294967295.0;
}
fn safe(v: vec3f) -> vec3f { return v / max(length(v), 0.0001); }
fn orb_at(i: u32, time: f32) -> vec3f {
    let a = f32(i) * 6.2831853 / max(f.orbs.x, 1.0) + time * f.orbs.w;
    return vec3f(cos(a), sin(a * 1.3) * 0.35, sin(a)) * f.orbs.y;
}
fn orb(i:u32)->vec3f { return orb_at(i,f.clock.y); }
fn orb_visual_radius(i:u32)->f32 {
    return optical_radius(i);
}
fn barrier_radius(i:u32)->f32 {
    // The render aspect is a camera-plane stretch; a surrounding sphere keeps
    // the obstacle independent of camera rotation. Bound extreme visual scales.
    return min(8.0,orb_visual_radius(i)*max(f.orb_style.z,1.0)+f.orb_field.y);
}
fn cell(p: vec3f) -> vec3i { return clamp(vec3i(floor((p / f.audio.w * 0.5 + 0.5) * 16.0)), vec3i(0), vec3i(15)); }
fn index(c: vec3i) -> u32 { return u32(c.x + c.y * 16 + c.z * 256); }
@compute @workgroup_size(128)
fn clear_grid(@builtin(global_invocation_id) gid: vec3u) {
    let i = gid.x; if i >= 4096u { return; }
    atomicStore(&grid[i].count, 0); atomicStore(&grid[i].px, 0); atomicStore(&grid[i].py, 0); atomicStore(&grid[i].pz, 0);
    atomicStore(&grid[i].vx, 0); atomicStore(&grid[i].vy, 0); atomicStore(&grid[i].vz, 0);
}
@compute @workgroup_size(128)
fn bin_particles(@builtin(global_invocation_id) gid: vec3u) {
    let i = gid.x; if i >= u32(f.clock.z) || f.clock.w > 0.5 { return; }
    let p = particles[i]; let c = index(cell(p.position.xyz));
    // Fixed-point sums stay below i32 capacity even with all 131k particles in one cell.
    let pos = vec3i(clamp(p.position.xyz, vec3f(-12.0), vec3f(12.0)) * 512.0);
    let vel = vec3i(clamp(p.velocity.xyz, vec3f(-8.0), vec3f(8.0)) * 512.0);
    atomicAdd(&grid[c].count, 1);
    atomicAdd(&grid[c].px, pos.x); atomicAdd(&grid[c].py, pos.y); atomicAdd(&grid[c].pz, pos.z);
    atomicAdd(&grid[c].vx, vel.x); atomicAdd(&grid[c].vy, vel.y); atomicAdd(&grid[c].vz, vel.z);
}
@compute @workgroup_size(128)
fn simulate(@builtin(global_invocation_id) gid: vec3u) {
    let i = gid.x; if i >= u32(f.clock.z) { return; }
    var p = particles[i].position.xyz; var v = particles[i].velocity.xyz;
    if f.clock.w > 0.5 {
        let angle = hash(i * 5u + 1u) * 6.2831853;
        let radius = 0.4 + pow(hash(i * 5u + 2u), 0.5) * f.audio.w * 0.65;
        p = vec3f(cos(angle) * radius, (hash(i * 5u + 3u) - 0.5) * radius * 0.6, sin(angle) * radius);
        v = vec3f(-sin(angle), 0.0, cos(angle)) * f.flock.w * 0.6;
    }
    var force = vec3f(0.0);
    // Bounded 27-cell aggregate flocking, rather than O(N²) pairwise neighbors.
    let c = cell(p); var center = vec3f(0.0); var heading = vec3f(0.0); var total = 0.0;
    for (var z = -1; z <= 1; z++) { for (var y = -1; y <= 1; y++) { for (var x = -1; x <= 1; x++) {
        let n = c + vec3i(x,y,z);
        if any(n < vec3i(0)) || any(n > vec3i(15)) { continue; }
        let k = index(n); let count = f32(atomicLoad(&grid[k].count));
        if count < 1.0 { continue; }
        let pos = vec3f(f32(atomicLoad(&grid[k].px)), f32(atomicLoad(&grid[k].py)), f32(atomicLoad(&grid[k].pz))) / 512.0;
        let vel = vec3f(f32(atomicLoad(&grid[k].vx)), f32(atomicLoad(&grid[k].vy)), f32(atomicLoad(&grid[k].vz))) / 512.0;
        let away = p - pos / count;
        let reach=f.audio.w/8.0*f.behavior.x;
        if f.behavior.x<1.749 && length(away)>reach {continue;}
        let forward=smoothstep(-0.2,0.4,dot(safe(v),safe(-away)));
        let awareness=mix(f.behavior.y,1.0,forward);
        center += pos*awareness; heading += vel*awareness; total += count*awareness;
        force += safe(away) * min((f.behavior.z*f.behavior.z) / (dot(away,away) + 0.05), 2.0) * f.flock.z * min(count / 64.0, 4.0) * 0.18;
    } } }
    if total > 1.0 {
        force += (center / total - p) * f.flock.x + (heading / total - v) * f.flock.y;
    }
    for (var j = 0u; j < u32(f.orbs.x); j++) {
        let d = orb(j) - p; let distance = max(length(d), 0.06); let dir = d / distance;
        let axis = safe(vec3f(sin(f32(j)*2.4)*0.5*f.circulation.x, 1.0, cos(f32(j)*1.7)*0.5*f.circulation.x));
        let strength = (1.0 + f.audio.x * f.audio.y * 3.0) / (1.0 + distance * distance * 0.3);
        force += (dir * f.forces.x + safe(cross(axis, dir)) * f.forces.y) * strength;
        force -= dir * f.forces.z * (1.0 + f.audio.x * 2.0) * exp(-distance * distance / max(f.orbs.z * f.orbs.z * 3.0, 0.01));
        if f.orb_field.x>0.5 {
            let gap=max(length(d)-barrier_radius(j),0.0);
            let weight=pow(clamp(1.0-gap/max(f.orb_field.z,0.001),0.0,1.0),f.orb_barrier.y);
            force += (-dir*f.orb_field.w+safe(cross(axis,dir))*f.orb_barrier.x)*weight;
        }
    }
    let phase=hash(i*7u+31u)*6.2831853;
    // Each particle has a different forcing phase. A shared smooth force field
    // alone synchronizes velocities and eventually collapses the flock to a line.
    let np=p*f.field.y; let nt=f.clock.y*f.field.z;
    force += vec3f(sin(np.y*2.3+nt+phase), cos(np.z*2.1-nt*0.7+phase*1.7), sin(np.x*1.7+nt*0.5+phase*2.3)) * f.forces.w;
    force -= p*f.dynamics.z;
    force.y += f.field.x-p.y*f.dynamics.w;
    let radius = length(p);
    force -= safe(p) * max(radius - f.audio.w * 0.72, 0.0) * f.field.w;
    // The distant safety enclosure is independent of artistic boundary strength.
    force -= safe(p)*max(radius-10.0,0.0)*20.0;
    force=safe(force)*min(length(force),f.behavior.w);
    let dt = min(f.clock.x, 0.0666667);
    v = (v + force * dt) * exp(-f.dynamics.x * dt);
    let direction=select(safe(v),vec3f(cos(phase),0.0,sin(phase)),length(v)<0.0001);
    v = direction * clamp(length(v),min(f.dynamics.y,f.flock.w),f.flock.w);
    let previous=p;
    p += v * dt;
    if length(p)>11.5 {p=safe(p)*11.5;v-=safe(p)*max(dot(v,safe(p)),0.0);}
    if f.orb_field.x>0.5 && f.orb_barrier.z>0.5 {
        // Sweep in the moving orb's reference frame: even a fast particle
        // crossing a small orb in one step must encounter the surface.
        for(var iteration=0u; iteration<4u; iteration++) {
            var corrected=false;
            for(var j=0u; j<u32(f.orbs.x); j++) {
                let center=orb(j); let old_center=orb_at(j,f.clock.y-dt);
                let r=barrier_radius(j)+0.0005;
                let start=previous-old_center;
                var relative=p-center;
                if iteration==0u && dot(start,start)>r*r {
                    let step=relative-start; let a=dot(step,step);
                    let b=dot(start,step); let c=dot(start,start)-r*r;
                    let discriminant=b*b-a*c;
                    if a>0.0000001 && b<0.0 && discriminant>=0.0 {
                        let hit=(-b-sqrt(discriminant))/a;
                        if hit>=0.0 && hit<=1.0 {
                            let normal=safe(start+step*hit);
                            let remaining=step*(1.0-hit);
                            relative=normal*r+remaining-normal*min(dot(remaining,normal),0.0);
                            p=center+relative;
                            corrected=true;
                            let orb_velocity=(center-old_center)/max(dt,0.0001);
                            v-=normal*min(dot(v-orb_velocity,normal),0.0);
                        }
                    }
                }
                if length(relative)<r {
                    var normal=safe(relative);
                    if length(relative)<0.0001 {normal=vec3f(cos(phase),0.0,sin(phase));}
                    p=center+normal*r;
                    corrected=true;
                    let orb_velocity=(center-old_center)/max(dt,0.0001);
                    v-=normal*min(dot(v-orb_velocity,normal),0.0);
                }
            }
            if !corrected {break;}
        }
        v=safe(v)*min(length(v),f.flock.w);
    }
    if f.path_clock.z>0.5 && i%u32(f.path_clock.w)==0u {
        let tracer=i/u32(f.path_clock.w);
        if tracer<512u { path_history[u32(f.path_clock.x)*512u+tracer]=vec4f(p,f.clock.y); }
    }
    next[i].position = vec4f(p, hash(i + 73u)); next[i].velocity = vec4f(v, 0.0);
}
// Raster entry points use a separate read-only binding layout for the current buffer.
struct Vertex { @builtin(position) clip: vec4f, @location(0) uv: vec2f, @location(1) color: vec3f, @location(2) kind: f32, @location(3) camera_point:vec3f }
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
@vertex
fn vertex(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> Vertex {
    let corners = array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1));
    let uv = corners[vi]; let is_orb = ii >= u32(f.clock.z);
    var world = particles[min(ii,u32(f.clock.z)-1u)].position.xyz;
    var velocity = particles[min(ii,u32(f.clock.z)-1u)].velocity.xyz;
    var tint = particles[min(ii,u32(f.clock.z)-1u)].position.w;
    if is_orb { world = orb(ii - u32(f.clock.z)); velocity = vec3f(0); tint = f32(ii-u32(f.clock.z)) / max(f.orbs.x,1.0); }
    let p = camera(world); let origin = project(p);
    let vel = camera(velocity); let direction = project(p + vel * 0.06) - origin;
    let pixel_direction=direction*f.output.xy;
    let tangent = select(vec2f(1.0,0.0), pixel_direction / max(length(pixel_direction), 0.00001), length(pixel_direction)>0.00001);
    let variation = mix(1.0, mix(0.2,1.8,hash(ii+317u)), f.particle_style.y);
    let size = variation * f.look.x * 2.0 / f.output.xy * select(clamp(f.camera.x/(f.camera.x+p.z),0.3,3.0),1.0,f.camera.z>0.5);
    let angle=select(f.particle_detail.y+f.clock.y*f.particle_detail.z+hash(ii+419u)*6.2831853*f.particle_detail.w,0.0,u32(f.particle_style.x)==6u);
    let rotated=vec2f(cos(angle)*uv.x*f.particle_style.z-sin(angle)*uv.y,sin(angle)*uv.x*f.particle_style.z+cos(angle)*uv.y);
    var offset = vec2f(-tangent.y,tangent.x) * rotated.y * size + tangent * rotated.x * size;
    offset += direction * rotated.x * f.look.y;
    if is_orb {
        let id=ii-u32(f.clock.z);
        let orb_size=orb_visual_radius(id);
        offset = uv * vec2f(f.orb_style.z,1.0) * orb_size * f.view.y * min(f.view.x, 1.0) * select(f.camera.y/max(f.camera.x+p.z,0.3),2.0/f.camera.x,f.camera.z>0.5) / vec2f(f.view.x,1.0);
    }
    var out: Vertex; out.clip = vec4f(origin + offset, 0.5, 1.0);
    if f.camera.x+p.z < 0.3 { out.clip = vec4f(5,5,0.5,1); }
    out.uv = uv;
    out.camera_point=p;
    let driver=select(color_driver(world,velocity,tint),tint,is_orb);
    out.color = palette(driver * f.particle_color.z + f.look.w + f.clock.y*f.particle_light.z, f.particle_color.x, f.color_start.rgb, f.color_end.rgb);
    let luminance=dot(out.color,vec3f(0.2126,0.7152,0.0722));
    out.color=max(mix(vec3f(luminance),out.color,f.particle_color.w),vec3f(0.0));
    let variation_light=mix(1.0,mix(0.1,1.9,hash(ii+547u)),f.particle_light.y);
    out.color *= f.look.z * (1.0 + f.audio.x * f.audio.z * 3.0) * select(f.particle_light.x*variation_light,f.particle_light.w,is_orb);
    if !is_orb {
        out.color*=optical_illumination(p);
        if u32(f.particle_color.x)==7u {
            let t=driver*f.particle_color.z+f.look.w+f.clock.y*f.particle_light.z;
            out.color*=mix(f.color_start.a,f.color_end.a,1.0-abs(fract(t*0.5)*2.0-1.0));
        }
    }
    out.kind = select(0.0,f32(ii)-f.clock.z+1.0,is_orb); return out;
}
@fragment
fn fragment(in: Vertex) -> @location(0) vec4f {
    let r = length(in.uv);
    // Signed silhouette distance permits crisp antialiased geometric particles.
    var distance=r-1.0;
    switch u32(f.particle_style.x) {
        case 2u: { distance=max(abs(in.uv.x),abs(in.uv.y))-1.0; }
        case 3u: { distance=abs(in.uv.x)+abs(in.uv.y)-1.0; }
        case 4u: { distance=max(r-1.0,1.0-f.particle_detail.x-r); }
        case 5u: { distance=min(max(abs(in.uv.x)-0.17,abs(in.uv.y)-1.0),max(abs(in.uv.x)-1.0,abs(in.uv.y)-0.17)); }
        case 6u: {
            let shaft=max(abs(in.uv.y)-0.20,max(-0.9-in.uv.x,in.uv.x-0.25));
            let head=max(abs(in.uv.y)-(1.0-in.uv.x)*0.85,max(-in.uv.x,in.uv.x-1.0));
            distance=min(shaft,head);
        }
        default: {}
    }
    let aa=max(fwidth(distance),0.001);
    var coverage=1.0-smoothstep(-aa,aa,distance);
    if u32(f.particle_style.x)==0u { coverage*=exp(-r*r*mix(1.5,7.5,f.particle_style.w)); }
    else { coverage*=1.0-smoothstep(-max(f.particle_style.w*0.75,aa),aa,distance); }
    let render_dt=select(f.clock.x,1.0/60.0,f.output.w>0.5 || f.clock.w>0.5);
    var intensity = coverage * 0.035 * min(render_dt * 60.0, 2.0) * (32768.0 / f.clock.z);
    if in.kind > 0.5 {
        if r > 1.0 { discard; }
        let normal = vec3f(in.uv, sqrt(max(1.0-r*r,0.0)));
        let light=vec3f(cos(f.orb_light.y)*sin(f.orb_light.x),sin(f.orb_light.y),cos(f.orb_light.y)*cos(f.orb_light.x));
        let diffuse = max(dot(normal,light),0.0);
        let highlight=pow(max(dot(normal,normalize(light+vec3f(0,0,1))),0.0),mix(256.0,2.0,f.orb_material.z));
        let rim = pow(1.0-normal.z, f.orb_surface.x);
        let shade=f.orb_material.x+diffuse*f.light.x+highlight*f.orb_material.y+rim*f.orb_material.w;
        // Transform the visible normal back into world space, so grids follow
        // the object while the camera moves. Patterns rotate independently.
        let world_normal=vec3f(dot(f.axis_x.xyz,normal),dot(f.axis_y.xyz,normal),dot(f.axis_z.xyz,normal));
        let a=f.orb_surface.w+f.clock.y*f.orb_light.z;
        let n=vec3f(cos(a)*world_normal.x+sin(a)*world_normal.z,world_normal.y,-sin(a)*world_normal.x+cos(a)*world_normal.z);
        let tilted=vec3f(n.x,cos(f.orb_light.w)*n.y-sin(f.orb_light.w)*n.z,sin(f.orb_light.w)*n.y+cos(f.orb_light.w)*n.z);
        let longitude=atan2(tilted.z,tilted.x)/6.2831853+0.5;
        let latitude=asin(clamp(tilted.y,-1.0,1.0))/3.14159265+0.5;
        let low=floor(f.orb_surface.y);
        let blend=fract(f.orb_surface.y);
        let lon_wave=mix(cos(longitude*6.2831853*low),cos(longitude*6.2831853*(low+1.0)),blend);
        let lat_wave=mix(cos(latitude*6.2831853*low),cos(latitude*6.2831853*(low+1.0)),blend);
        let threshold=cos(f.orb_surface.z*3.14159265);
        let wire= smoothstep(threshold-0.08,threshold+0.02,max(lon_wave,lat_wave));
        let bands=smoothstep(threshold-0.08,threshold+0.02,lat_wave);
        intensity=shade;
        switch u32(f.orb_style.x) {
            case 1u: { intensity=1.0; }
            case 2u: { intensity=smoothstep(1.0-f.orb_surface.z-0.02,1.0-f.orb_surface.z+0.02,r); }
            case 3u: { intensity=shade*wire; }
            case 4u: { intensity=shade*bands; }
            case 5u: { intensity=shade*(0.2+0.8*pow(0.5+0.5*sin((tilted.x+sin(tilted.y*3.0))*f.orb_surface.y+sin(tilted.z*f.orb_surface.y)),2.0)); }
            case 6u: { intensity=exp(-r*r*5.0); }
            case 7u: {
                let rim_direction=normal.xy/max(length(normal.xy),0.001);
                let lamp_direction=light.xy/max(length(light.xy),0.001);
                let crescent=0.03+0.97*pow(clamp(0.5+0.5*dot(rim_direction,lamp_direction),0.0,1.0),2.0);
                let edge=exp(-pow((r-0.94)/max(f.orb_surface.z*0.45,0.012),2.0));
                intensity=f.orb_material.x*normal.z+edge*f.orb_material.w*crescent;
            }
            case 8u: {
                let veins=0.65+0.35*sin(longitude*37.6991+sin(latitude*25.1327)+f.clock.y*0.4);
                intensity=(0.15+0.85*normal.z)*veins+pow(r,6.0)*f.orb_material.w*0.3;
            }
            default: {}
        }
        intensity *= (1.0-smoothstep(0.92,1.0,r))*f.orb_style.w*optical_alpha(u32(in.kind-1.0));
        intensity *= select(1.0, 1.0-exp2(-render_dt/max(f.light.w,0.001)), f.light.w>0.001);
        var color=in.color;
        if f.orb_color.x>0.5 {
            let id=u32(in.kind-1.0+0.1);
            let along=mix(f32(id)/max(f.orbs.x-1.0,1.0),latitude,f.orb_tint.z);
            color=palette(along*f.orb_color.w+f.orb_color.z+f.clock.y*f.orb_tint.y,f.orb_color.y,f.orb_start.rgb,f.orb_end.rgb);
            if u32(f.orb_color.x)==2u { color=pow(f.orb_start.rgb,vec3f(2.2)); }
            if u32(f.orb_color.x)==3u { color=pow(f.orb_colors[min(id,4u)].rgb,vec3f(2.2)); }
            color *= f.look.z*(1.0+f.audio.x*f.audio.z*3.0)*f.particle_light.w;
        }
        let gray=dot(color,vec3f(0.2126,0.7152,0.0722));
        color=max(mix(vec3f(gray),color,f.orb_tint.x),vec3f(0.0));
        let id=u32(in.kind-1.0+0.1);
        let radius=orb_visual_radius(id);
        let surface=in.camera_point+vec3f(in.uv.x*radius*f.orb_style.z,in.uv.y*radius,-normal.z*radius);
        intensity*=optical_visibility(surface,id)*(1.0+f.orb_flare.z*2.0);
        return vec4f(color*intensity,0.0);
    }
    return vec4f(in.color * intensity*optical_visibility(in.camera_point,99u), 0.0);
}
