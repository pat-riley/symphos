struct Frame {clock:vec4f,geometry:vec4f,motion:vec4f,grains:vec4f,recycling:vec4f,surface:vec4f,light:vec4f,direction:vec4f,view:vec4f,axis_x:vec4f,axis_y:vec4f,axis_z:vec4f,camera:vec4f,color_a:vec4f,color_b:vec4f,plate_color:vec4f,color:vec4f,output:vec4f}
@group(0) @binding(0) var<uniform> f:Frame;
@group(0) @binding(1) var<storage,read> field:array<vec4f>;
// Render pipeline uses the same binding as read-only; compute alone writes it.
@group(0) @binding(2) var<storage,read_write> grains:array<vec4f>;
fn hash(v:u32)->f32 {var x=v;x=((x>>16u)^x)*0x45d9f3bu;x=((x>>16u)^x)*0x45d9f3bu;x=(x>>16u)^x;return f32(x&0x00ffffffu)/16777216.0;}
fn sample_field(p:vec2f)->vec4f {let uv=clamp((p+1.0)*32.0,vec2f(0),vec2f(64));let i=vec2u(min(floor(uv),vec2f(63)));let t=uv-vec2f(i);return mix(mix(field[i.y*65u+i.x],field[i.y*65u+i.x+1u],t.x),mix(field[(i.y+1u)*65u+i.x],field[(i.y+1u)*65u+i.x+1u],t.x),t.y);}
@compute @workgroup_size(128) fn simulate(@builtin(global_invocation_id) id:vec3u){
 let i=id.x;if i>=u32(f.output.w){return;}
 var g=grains[i];
 if f.clock.z>0.5 {let seed=i+u32(f.clock.w)*7919u;g=vec4f(hash(seed*3u+1u)*2.0-1.0,hash(seed*3u+2u)*2.0-1.0,0,0);if f.geometry.x>0.5 {let r=sqrt(hash(seed+12u))*0.98;let a=hash(seed+19u)*6.283185;g.x=r*cos(a);g.y=r*sin(a);}}
 let steps=u32(ceil(f.clock.x*120.0));let dt=f.clock.x/f32(max(steps,1u));
 for(var step=0u;step<steps;step++){
    let field_value=sample_field(g.xy);let grad=field_value.zw;
    var force=-grad*f.motion.x;
    let activity=clamp(sqrt(max(field_value.x,0.0))*3.0,0.0,1.0);
    let t=f.clock.y+f32(step)*dt;
    // Effective grain transport, not individual grain impacts or air flow.
    let random=vec2f(hash(i*7u+u32(t*120.0)*13u),hash(i*11u+u32(t*120.0)*17u))*2.0-1.0;
    force+=random*f.motion.z*activity*sqrt(120.0);
    force+=vec2f(sin(g.y*9.0+t),cos(g.x*7.0-t))*f.motion.w*activity;
    g=vec4f(g.xy,(g.zw+force*dt)*exp(-f.motion.y*dt));
    let speed=length(g.zw);g=vec4f(g.xy,g.zw*min(1.0,0.7/max(speed,0.0001)));g=vec4f(g.xy+g.zw*dt,g.zw);
    // Recycle before grains settle on the low-energy rim. A central disk
    // avoids a single pile at the origin, and cleared velocity prevents jumps.
    let edge_distance=select(max(abs(g.x),abs(g.y)),length(g.xy),f.geometry.x>0.5);
    if f.recycling.x>0.5 && f.camera.w>0.000001 && edge_distance>=f.recycling.y {
        let seed=i*31u+u32(t*120.0)*7919u+u32(f.clock.w)*104729u+step;
        let radius=sqrt(hash(seed+41u))*f.recycling.z;
        let angle=hash(seed+67u)*6.283185;
        g=vec4f(radius*cos(angle),radius*sin(angle),0,0);
    }
    if f.geometry.x>0.5 {let r=length(g.xy);if r>0.99{let normal=g.xy/r;g=vec4f(normal*0.99,g.zw-normal*max(dot(g.zw,normal),0.0)*1.4);}}
    else {if abs(g.x)>0.99{g.x=clamp(g.x,-0.99,0.99);g.z*=-0.25;}if abs(g.y)>0.99{g.y=clamp(g.y,-0.99,0.99);g.w*=-0.25;}}
 }
 grains[i]=g;
}
fn project(p:vec3f)->vec4f {let q=f.axis_x.xyz*p.x+f.axis_y.xyz*p.y+f.axis_z.xyz*p.z;let depth=max(0.2,f.camera.x+q.z);let divisor=select(depth,f.camera.x,f.camera.z>0.5);let scale=f.camera.y*f.view.y*min(f.view.x,1.0)/divisor;return vec4f(q.x*scale/f.view.x+f.view.z,-q.y*scale+f.view.w,clamp(depth/50.0,0.001,0.999),1.0);}
fn world(p:vec2f,offset:f32)->vec3f {let v=sample_field(p);return vec3f(p.x,v.y*f.geometry.w+offset,p.y*f.geometry.y);}
fn tone(c:vec3f)->vec3f {let rgb=clamp(c,vec3f(0),vec3f(1));return select(rgb,pow(rgb,vec3f(2.2)),f.output.z>0.5);}
fn palette(t:f32)->vec3f {return mix(f.color_a.rgb,f.color_b.rgb,clamp(pow(clamp(t*f.color.y+f.color.z,0.0,1.0),f.color.w),0.0,1.0));}
struct V {@builtin(position) p:vec4f,@location(0) uv:vec2f,@location(1) local:vec2f,@location(2) shade:f32}
@vertex fn plate_vertex(@builtin(vertex_index) i:u32)->V {let cell=i/6u;let corner=array<vec2f,6>(vec2f(0,0),vec2f(1,0),vec2f(0,1),vec2f(0,1),vec2f(1,0),vec2f(1,1))[i%6u];let uv=(vec2f(f32(cell%64u),f32(cell/64u))+corner)/32.0-1.0;var o:V;o.p=project(world(uv,0));o.uv=uv;o.local=vec2f(0);o.shade=1;return o;}
@fragment fn plate_fragment(v:V)->@location(0) vec4f {
 if f.geometry.x>0.5&&length(v.uv)>1.0{discard;}
 let value=sample_field(v.uv);let amplitude=sqrt(max(value.x,0.0));
 var color=f.plate_color.rgb;var opacity=f.surface.x;
 if f.geometry.z>0.5 {
    let h=0.012;let dx=(sample_field(v.uv+vec2f(h,0)).y-sample_field(v.uv-vec2f(h,0)).y)/(2.0*h);
    let dy=(sample_field(v.uv+vec2f(0,h)).y-sample_field(v.uv-vec2f(0,h)).y)/(2.0*h*f.geometry.y);
    let normal=normalize(vec3f(-dx*f.geometry.w,1,-dy*f.geometry.w));let light=normalize(f.direction.xyz);
    let view_direction=normalize(-vec3f(f.axis_x.z,f.axis_y.z,f.axis_z.z));
    let facing_normal=normal*select(-1.0,1.0,dot(normal,view_direction)>=0.0);
    let diffuse=max(dot(facing_normal,light),0.0);
    let halfway=normalize(light+view_direction+vec3f(0.00001));
    let spec=pow(max(dot(facing_normal,halfway),0.0),mix(100.0,3.0,f.light.w));
    color=palette(select(amplitude*2.0,value.y*0.5+0.5,f.color.x>0.5))*(f.light.x+f.light.y*diffuse)+vec3f(f.light.z*spec);
    if f.geometry.z>1.5&&f.geometry.z<2.5 {let node=exp(-amplitude*amplitude/max(f.surface.z*f.surface.z,0.00001));let contour=pow(0.5+0.5*cos(amplitude*f.surface.w*6.283185),18.0)*0.3;color=mix(f.plate_color.rgb,palette(amplitude*2.0),clamp(node+contour,0.0,1.0)*clamp(f.camera.w*200.0,0.0,1.0));}
 }
 return vec4f(tone(color*f.surface.y),opacity);
}
@vertex fn grain_vertex(@builtin(vertex_index) vertex:u32,@builtin(instance_index) i:u32)->V {
 let g=grains[i];let corner=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1))[vertex];
 var o:V;o.p=project(world(g.xy,0.004));let size=f.grains.x*mix(1.0,0.45+hash(i+17u)*1.1,f.grains.y);o.p=vec4f(o.p.xy+corner*size*2.0/f.output.xy,o.p.zw);o.p.z-=0.0002;o.uv=g.xy;o.local=corner;o.shade=hash(i+53u);return o;
}
@fragment fn grain_fragment(v:V)->@location(0) vec4f {
 let radius=length(v.local);if radius>1.0{discard;}
 var alpha=(1.0-smoothstep(0.65,1.0,radius))*f.grains.z;
 if f.direction.w>0.5{alpha*=exp(-radius*radius*3.0);}
 let energy=sqrt(max(sample_field(v.uv).x,0.0));let t=select(v.shade,energy*3.0,f.color.x>0.5);
 let glow=1.0+f.grains.w*(1.0-radius);return vec4f(tone(palette(t)*f.surface.y*glow),alpha);
}
