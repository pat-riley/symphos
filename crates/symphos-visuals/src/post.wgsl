struct Frame { clock:vec4f, flock:vec4f, forces:vec4f, orbs:vec4f, look:vec4f, light:vec4f, view:vec4f, axis_x:vec4f, axis_y:vec4f, axis_z:vec4f, audio:vec4f, output:vec4f, particle_style:vec4f, particle_detail:vec4f, particle_color:vec4f, particle_light:vec4f, color_start:vec4f, color_end:vec4f, orb_style:vec4f, orb_shape:vec4f, orb_material:vec4f, orb_surface:vec4f, orb_light:vec4f, orb_color:vec4f, orb_tint:vec4f, orb_start:vec4f, orb_end:vec4f, camera:vec4f, behavior:vec4f, dynamics:vec4f, field:vec4f, circulation:vec4f, paths:vec4f, path_style:vec4f, path_clock:vec4f, orb_field:vec4f, orb_barrier:vec4f, orb_optics:vec4f, orb_corona:vec4f, orb_flare:vec4f, orb_colors:array<vec4f,5>, }
@group(0) @binding(0) var<uniform> f: Frame;
@group(0) @binding(1) var image: texture_2d<f32>;
@group(0) @binding(2) var glow: texture_2d<f32>;
@group(0) @binding(3) var filtering: sampler;
struct Vertex { @builtin(position) position:vec4f, @location(0) uv:vec2f }
@vertex fn vertex(@builtin(vertex_index) i:u32) -> Vertex {
    let p = array<vec2f,3>(vec2f(-1,-1),vec2f(3,-1),vec2f(-1,3));
    var o:Vertex; o.position=vec4f(p[i],0,1); o.uv=p[i]*vec2f(0.5,-0.5)+0.5; return o;
}
@fragment fn history(i:Vertex)->@location(0) vec4f {
    let decay=select(exp2(-f.clock.x / max(f.light.w,0.001)),0.0,f.clock.w>0.5 || f.output.w>0.5 || f.light.w<0.001);
    return vec4f(textureSample(image,filtering,i.uv).rgb * decay,1);
}
fn blur(uv:vec2f, direction:vec2f)->vec4f {
    let step=direction/vec2f(textureDimensions(image));
    var color=textureSample(image,filtering,uv).rgb*0.227027;
    color+=(textureSample(image,filtering,uv+step*1.384615).rgb+textureSample(image,filtering,uv-step*1.384615).rgb)*0.316216;
    color+=(textureSample(image,filtering,uv+step*3.230769).rgb+textureSample(image,filtering,uv-step*3.230769).rgb)*0.070270;
    return vec4f(color,1);
}
@fragment fn horizontal(i:Vertex)->@location(0) vec4f { return blur(i.uv,vec2f(4,0)); }
@fragment fn vertical(i:Vertex)->@location(0) vec4f { return blur(i.uv,vec2f(0,2)); }

@fragment fn composite(i:Vertex)->@location(0) vec4f {
    var warped=i.uv; var chroma=vec2f(0); var atmosphere=vec3f(0);
    if abs(f.orb_optics.y)>0.0001 || f.orb_corona.x>0.0 || f.orb_corona.z>0.0 {
        for(var j=0u;j<u32(f.orbs.x);j++) {
            let center=optical_center(j);
            if f.camera.x+center.z<0.3 {continue;}
            let ndc=optical_projection(center);
            let origin=ndc*vec2f(0.5,-0.5)+0.5;
            let radius=optical_radius(j);
            let scale=select(f.camera.y/max(f.camera.x+center.z,0.3),2.0/f.camera.x,f.camera.z>0.5);
            let extent=radius*f.view.y*min(f.view.x,1.0)*scale*0.5;
            let radii=vec2f(extent*f.orb_style.z/f.view.x,extent);
            let q=(i.uv-origin)/max(radii,vec2f(0.00001));
            let r=length(q);
            if r>max(f.orb_optics.z,1.0+f.orb_corona.w) || r<0.02 {continue;}
            let lens_envelope=smoothstep(0.7,1.0,r)*(1.0-smoothstep(1.0,f.orb_optics.z,r));
            let shift=q*radii*(f.orb_optics.y*0.32*lens_envelope/max(r*r,0.5));
            warped+=shift;chroma+=shift*f.orb_optics.w*0.2;
            let outside=max(r-1.0,0.0);
            let edge=smoothstep(0.93,1.01,r);
            let halo=exp(-outside/max(f.orb_corona.y,0.01))*edge*f.orb_corona.x;
            let angle=atan2(q.y,q.x)+f.clock.y*f.orb_flare.y+f32(j)*1.7;
            let low=floor(f.orb_flare.x);let blend=fract(f.orb_flare.x);
            let curved=angle+0.06*sin(angle*7.0+outside*2.2)*smoothstep(0.0,2.0,outside);
            let strands=mix(cos(curved*low),cos(curved*(low+1.0)),blend);
            let irregular=pow(0.5+0.5*strands,12.0)*(0.25+0.75*pow(0.5+0.5*sin(angle*7.0+sin(angle*3.0)),2.0));
            let local_length=f.orb_corona.w*(0.25+0.75*pow(0.5+0.5*sin(angle*5.0+sin(angle*11.0)),2.0));
            let rays=irregular*exp(-outside*3.0/max(local_length,0.1))*edge
                *(1.0-smoothstep(local_length*0.7,local_length,outside))*f.orb_corona.z;
            let crescent=0.04+0.96*pow(0.5+0.5*cos(atan2(q.y,q.x)-f.orb_light.x),3.0);
            let emission_point=center+vec3f(q.x*radius*f.orb_style.z,-q.y*radius,0.0);
            let visibility=optical_visibility(emission_point,j);
            atmosphere+=optical_color(j)*(halo*crescent+rays)*(1.0+f.orb_flare.z*3.0)*visibility;
        }
    }
    warped=clamp(warped,vec2f(0),vec2f(1));
    var color=textureSampleLevel(image,filtering,warped,0.0).rgb;
    if dot(chroma,chroma)>0.00000001 {
        color.r=textureSampleLevel(image,filtering,clamp(warped+chroma,vec2f(0),vec2f(1)),0.0).r;
        color.b=textureSampleLevel(image,filtering,clamp(warped-chroma,vec2f(0),vec2f(1)),0.0).b;
    }
    color+=textureSampleLevel(glow,filtering,warped,0.0).rgb*f.light.y+atmosphere;
    color=vec3f(1)-exp(-color*f.light.z);
    if f.output.z < 0.5 { color=pow(color,vec3f(1.0/2.2)); }
    return vec4f(color,1);
}
