struct Layer { transform:vec4f, rotation:vec4f, style:vec4f, a:vec4f, b:vec4f, c:vec4f, gradient:array<vec4f,8>, palette:vec4f }
struct Frame {
    clock:vec4f, geometry:vec4f, arrangement:vec4f, motion:vec4f, response:vec4f,
    ripple:vec4f, look:vec4f, optics:vec4f, color:vec4f, light:vec4f,
    view:vec4f, camera:vec4f, axis_x:vec4f, axis_y:vec4f, axis_z:vec4f, output:vec4f,
    sculpture:vec4f, fragments:vec4f, feedback:vec4f, feedback_shift:vec4f,
    layers:array<Layer,8>
}
struct Grain { offset:vec4f, velocity:vec4f }
@group(0) @binding(0) var<uniform> f:Frame;
@group(0) @binding(1) var<storage,read> spectrum:array<f32>;
@group(0) @binding(2) var<storage,read_write> grains:array<Grain>;
fn hash(n:u32)->f32 {var x=n*747796405u+2891336453u;x=((x>>((x>>28u)+4u))^x)*277803737u;x=(x>>22u)^x;return f32(x)/4294967295.0;}
fn column_count()->u32 {return select(128u,u32(f.sculpture.y),f.sculpture.x>0.5);}
fn row_count()->u32 {
    let per=u32(f.output.w)/u32(f.geometry.w);
    if f.sculpture.x>0.5 {return max(2u,u32(f32(per)*(1.0-f.fragments.x))/column_count());}
    return max(2u,(per+127u)/128u);
}
fn dissolved(id:u32)->bool {return f.sculpture.x>0.5 && id/u32(f.geometry.w)>=row_count()*column_count();}
fn uv_for(id:u32)->vec2f {
    if dissolved(id) {return vec2f(hash(id+991u),hash(id+993u));}
    let n=id/u32(f.geometry.w);let columns=column_count();
    return vec2f(f32(n%columns)/f32(columns-1u), f32(n/columns)/f32(row_count()-1u));
}
fn level(uv:vec2f)->f32 {
    let p=clamp(uv,vec2f(0),vec2f(1))*vec2f(95,63);let lo=vec2u(p);let hi=min(lo+vec2u(1),vec2u(95,63));let t=fract(p);
    return mix(mix(spectrum[lo.y*96u+lo.x],spectrum[lo.y*96u+hi.x],t.x),mix(spectrum[hi.y*96u+lo.x],spectrum[hi.y*96u+hi.x],t.x),t.y);
}
fn base(uv:vec2f,layer:u32,amp:f32)->vec3f {
    if f.sculpture.x>0.5 {
        let v=uv.y;let tail=smoothstep(0.5,1.0,v);let t=f.clock.y*f.response.w*f.layers[layer].style.z;
        let current=level(vec2f(uv.x, min(0.9,f32(layer)*0.08+v*0.15)));
        let fold=f.sculpture.w*tail*current;
        return vec3f((uv.x-0.5)*f.geometry.x+f.sculpture.z*tail+sin(v*10.0+uv.x*9.0-t*0.3)*fold,
            current*f.geometry.z*(1.0-min(v/0.55,1.0))-f.geometry.z*0.3-tail*f.fragments.w+cos(v*12.0+uv.x*6.0+t*0.3)*fold,
            (max(v-0.55,0.0)/0.45-0.5)*f.geometry.y+(f32(layer)-(f.geometry.w-1.0)*0.5)*f.arrangement.x+sin(v*9.0+t*0.2)*fold);
    }
    let centered=f32(layer)-(f.geometry.w-1.0)*0.5;
    let y=amp*f.geometry.z+centered*f.arrangement.x;
    let phase=uv.x*6.2831853;
    let v=uv.y-0.5;
    var p=vec3f((uv.x-0.5)*f.geometry.x,y,v*f.geometry.y);
    let mode=u32(f.arrangement.w);
    if mode==1u {let r=f.arrangement.y+uv.y*f.geometry.y*0.35;p=vec3f(cos(phase)*r,y,sin(phase)*r);}
    if mode==2u {let angle=phase*f.arrangement.z+uv.y*2.0;let r=f.arrangement.y*(0.3+uv.x);p=vec3f(cos(angle)*r,y+v*f.geometry.y,sin(angle)*r);}
    if mode==3u {let r=f.arrangement.y+amp*f.geometry.z+centered*f.arrangement.x;p=vec3f(cos(phase)*r,sin(phase)*r,v*f.geometry.y);}
    let wave=sin((uv.x+uv.y*0.7)*f.ripple.y*6.2831853-f.clock.y*f.ripple.z*f.layers[layer].style.z);
    p.y+=amp*wave*f.ripple.x;
    let twist=f.ripple.w*v;
    p=vec3f(p.x*cos(twist)-p.z*sin(twist),p.y,p.x*sin(twist)+p.z*cos(twist));
    return p;
}
fn transform(p:vec3f,l:Layer)->vec3f {
    let a=l.rotation.x;let b=l.rotation.y;
    let t=vec3f(p.x,p.y*cos(b)-p.z*sin(b),p.y*sin(b)+p.z*cos(b));
    return vec3f(t.x*cos(a)-t.z*sin(a),t.y,t.x*sin(a)+t.z*cos(a))*l.transform.w+l.transform.xyz;
}
@compute @workgroup_size(128)
fn simulate(@builtin(global_invocation_id) gid:vec3u) {
    let id=gid.x;if id>=u32(f.output.w){return;}
    if f.clock.z>0.5 {grains[id].offset=vec4f(0);grains[id].velocity=vec4f(0);}
    let dt=clamp(f.clock.x,0.0,0.1);if dt==0.0{return;}
    let uv=uv_for(id);let layer=id%u32(f.geometry.w);let amp=level(uv);
    let strength=max(0.0,(f.response.x-f.response.y)/(1.0-f.response.y));
    let can_drift=select(hash(id+347u)<f.clock.w,dissolved(id),f.sculpture.x>0.5);
    let drive=f.motion.x*strength*(0.25+amp*0.75)*(1.0+strength*f.response.z)*select(0.0,1.0,can_drift);
    var o=grains[id].offset.xyz;var velocity=grains[id].velocity.xyz;
    let steps=u32(ceil(dt*120.0));let h=dt/f32(steps);
    for(var step=0u;step<steps;step++) {
        let t=(f.clock.y-dt+f32(step)*h)*f.response.w*f.layers[layer].style.z;
        let seed=vec3f(hash(id*3u+1u),hash(id*3u+2u),hash(id*3u+3u))*6.2831853;
        let p=base(uv,layer,amp)+o;
        let flow=vec3f(sin(p.y*f.motion.w+t+seed.x),sin(p.z*f.motion.w-t*0.8+seed.y),cos(p.x*f.motion.w+t*0.6+seed.z));
        velocity+=(flow*drive*12.0-o*f.motion.y)*h;
        velocity*=exp(-f.motion.z*h);
        velocity*=min(1.0,6.0/max(length(velocity),0.001));
        o+=velocity*h;
        o*=min(1.0,4.0/max(length(o),0.001));
    }
    grains[id].offset=vec4f(o,0);grains[id].velocity=vec4f(velocity,0);
}
// RENDER_START: simulation is removed from the read-only vertex shader module.
fn palette_color(l:Layer,t:f32)->vec3f {
    let blend=clamp(l.rotation.z,0.0,1.0);
    if l.palette.x<2.0 {
        // Preserve the authored three-color distribution of older presets.
        let width=max(0.001,blend);
        let first=mix(l.a.xyz,l.b.xyz,smoothstep(0.58-0.3*width,0.58+0.3*width,t));
        return mix(first,l.c.xyz,smoothstep(0.9-0.2*width,0.9+0.1*width,t));
    }
    let n=u32(clamp(l.palette.x,2.0,8.0));
    var x=t*f32(n-1u);var i=min(u32(x),n-2u);var next=i+1u;
    if l.palette.y>0.5 { x=fract(t)*f32(n);i=u32(x)%n;next=(i+1u)%n; }
    let fraction=x-f32(i);
    let weight=mix(select(0.0,1.0,fraction>=0.5),fraction,blend);
    return mix(l.gradient[i].rgb,l.gradient[next].rgb,weight);
}
struct V { @builtin(position) position:vec4f,@location(0) uv:vec2f,@location(1) color:vec4f,@location(2) blur:f32,@location(3) strand:f32 }
@vertex fn vertex(@builtin(vertex_index) vi:u32,@builtin(instance_index) id:u32)->V {
    var o:V;o.position=vec4f(2,2,2,1);o.uv=vec2f(0);o.color=vec4f(0);o.blur=0.0;o.strand=0.0;
    let layer=id%u32(f.geometry.w);let l=f.layers[layer];
    if l.rotation.w<0.5 || hash(id+900u)>f.look.w*l.style.y {return o;}
    let uv=uv_for(id);let amp=level(uv);
    let is_cloud=dissolved(id);
    var point=base(uv,layer,amp)+grains[id].offset.xyz;
    if is_cloud {
        let tail_uv=vec2f(uv.x,0.55+uv.y*0.45);
        point=base(tail_uv,layer,level(tail_uv))+grains[id].offset.xyz*2.0;
        let spread=vec3f(hash(id+213u),hash(id+215u),hash(id+217u))*2.0-1.0;
        point+=spread*f.fragments.y*(0.2+amp);
    }
    let world=transform(point,l);
    let p=world.x*f.axis_x.xyz+world.y*f.axis_y.xyz+world.z*f.axis_z.xyz;
    let depth=f.camera.x+p.z;if depth<0.25 {return o;}
    let focal=1.0/tan(f.camera.y*0.5);
    let proj=select(focal/depth,focal/f.camera.x,f.camera.z>0.5)*f.view.y*min(f.view.x,1.0);
    let position=vec2f(p.x/f.view.x,-p.y)*proj+f.view.zw;
    let perspective=select(clamp(f.optics.x/depth,0.15,8.0),1.0,f.camera.z>0.5);
    let radius=select(1.0,f.arrangement.y,is_cloud)*f.look.x*l.style.x*mix(1.0,mix(0.35,1.8,hash(id+55u)),f.look.y)*perspective*(1.0+amp*f.light.z);
    let defocus=min(f.optics.z,abs(depth-f.optics.x)/max(depth,0.25)*f.optics.y*24.0);
    let core=radius+defocus;let outer=core*(1.15+f.light.x*0.5);
    let pixel_scale=min(f.output.x,f.output.y)/1080.0;
    let corner=array<vec2f,6>(vec2f(-1,-1),vec2f(1,-1),vec2f(-1,1),vec2f(-1,1),vec2f(1,-1),vec2f(1,1))[vi];
    o.position=vec4f(position+corner*outer*pixel_scale*2.0/f.output.xy,0,1);o.uv=corner*(1.15+f.light.x*0.5);
    if f.sculpture.x>0.5 && !is_cloud {
        let rows=row_count();
        let next_uv=vec2f(uv.x,min(1.0,uv.y+1.0/f32(rows-1u)));
        let next_world=transform(base(next_uv,layer,level(next_uv))+grains[id].offset.xyz,l);
        let next_p=next_world.x*f.axis_x.xyz+next_world.y*f.axis_y.xyz+next_world.z*f.axis_z.xyz;
        let next_depth=f.camera.x+next_p.z;
        if next_depth<0.25 {return o;}
        let next_proj=select(focal/next_depth,focal/f.camera.x,f.camera.z>0.5)*f.view.y*min(f.view.x,1.0);
        let end=vec2f(next_p.x/f.view.x,-next_p.y)*next_proj+f.view.zw;
        let delta=(end-position)*f.output.xy;
        let normal=vec2f(-delta.y,delta.x)/max(length(delta),0.001);
        o.position=vec4f(mix(position,end,corner.y*0.5+0.5)+normal*corner.x*max(0.35,radius*f.fragments.z)*pixel_scale*2.0/f.output.xy,0,1);
        o.uv=corner;o.strand=1.0;o.blur=0.0;
    }
    var driver=hash(id+771u);
    if f.color.x>0.5 && f.color.x<1.5 {driver=uv.x;}
    if f.color.x>1.5 && f.color.x<2.5 {driver=amp;}
    if f.color.x>2.5 {driver=f32(layer)/max(1.0,f.geometry.w-1.0);}
    let color_position=clamp(driver*f.color.y+f.color.z,0.0,1.0);
    var rgb=palette_color(l,color_position);
    rgb=max(vec3f(0),mix(vec3f(dot(rgb,vec3f(0.2126,0.7152,0.0722))),rgb,f.color.w));
    // Spread light across the bokeh disk rather than brightening distant blur.
    let energy=select(pow(radius/max(core,0.01),1.15),1.0,o.strand>0.5);
    // Normalize new light against persistence so 60/120 Hz do not double brightness.
    let injection=select(1.0,1.0-exp(-max(f.clock.x,0.0083333)/max(f.feedback.x,0.001)),f.feedback.x>0.0 && f.feedback_shift.w<0.5);
    o.color=vec4f(rgb*f.light.y*(1.0+amp*f.light.w)*select(1.0,f.arrangement.z,is_cloud),f.look.z*l.style.w*energy*injection*select(1.0,mix(f.arrangement.w,1.0,smoothstep(0.0,0.3,amp)),f.sculpture.x>0.5));
    o.blur=clamp(defocus/max(core,0.01),0.0,1.0);
    return o;
}
@fragment fn fragment(v:V)->@location(0) vec4f {
    var r=length(v.uv);let shape=u32(f.optics.w);
    if shape==1u {r=max(abs(v.uv.x),abs(v.uv.y));}
    if shape==2u {r=abs(v.uv.x)+abs(v.uv.y);}
    if shape==3u {r=abs(length(v.uv)-0.7)*3.0;}
    if v.strand>0.5 {r=abs(v.uv.x);}
    let soft=clamp(f.camera.w+v.blur*0.8,0.02,1.0);
    let disk=1.0-smoothstep(1.0-soft,1.0,r);
    let halo=exp(-r*r*2.8)*f.light.x*0.3;
    let a=clamp((disk+halo)*v.color.a,0.0,1.0);
    var rgb=v.color.rgb;
    if f.output.z>0.5 {rgb=pow(rgb,vec3f(2.2));}
    return vec4f(rgb*a,a);
}
