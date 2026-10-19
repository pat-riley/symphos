// Shared analytic orb geometry for particle/path visibility and optical effects.
// No texture or storage dependency: all shapes derive from the same frame data.
fn optical_hash(n:u32)->f32 {
    var x=n*747796405u+2891336453u;
    x=((x>>((x>>28u)+4u))^x)*277803737u;
    return f32((x>>22u)^x)/4294967295.0;
}
fn optical_center(i:u32)->vec3f {
    let a=f32(i)*6.2831853/max(f.orbs.x,1.0)+f.clock.y*f.orbs.w;
    let world=vec3f(cos(a),sin(a*1.3)*0.35,sin(a))*f.orbs.y;
    return f.axis_x.xyz*world.x+f.axis_y.xyz*world.y+f.axis_z.xyz*world.z;
}
fn optical_radius(i:u32)->f32 {
    return f.orbs.z*f.orb_style.y*mix(1.0,mix(0.5,1.5,optical_hash(i+719u)),f.orb_shape.x)
        *(1.0+f.orb_shape.y*sin(f.clock.y*f.orb_shape.z*6.2831853+f32(i)*1.7))
        *(1.0+f.audio.x*f.orb_shape.w)*(1.0+f.orb_flare.z*f.orb_flare.w);
}
fn optical_projection(p:vec3f)->vec2f {
    let scale=select(f.camera.y/max(f.camera.x+p.z,0.3),2.0/f.camera.x,f.camera.z>0.5);
    return p.xy*f.view.y*min(f.view.x,1.0)*scale/vec2f(f.view.x,1.0)+f.view.zw;
}
fn optical_visibility(point:vec3f,ignore:u32)->f32 {
    if f.orb_optics.x<=0.0 || f.orb_style.w<=0.0 {return 1.0;}
    var origin=vec3f(0,0,-f.camera.x);
    var direction=normalize(point-origin);
    if f.camera.z>0.5 {origin=vec3f(point.xy,-f.camera.x);direction=vec3f(0,0,1);}
    let particle_distance=length(point-origin);
    var visible=1.0;
    for(var j=0u;j<u32(f.orbs.x);j++) {
        if j==ignore {continue;}
        let center=optical_center(j);
        let radius=optical_radius(j);
        let axes=vec3f(radius*f.orb_style.z,radius,radius);
        let relative=(origin-center)/axes;
        let ray=direction/axes;
        let a=dot(ray,ray);let b=dot(relative,ray);let c=dot(relative,relative)-1.0;
        let disc=b*b-a*c;
        if disc>0.0 {
            let hit=(-b-sqrt(disc))/a;
            if hit>0.0 && hit<particle_distance-0.001 {visible*=1.0-f.orb_optics.x*f.orb_style.w;}
        }
    }
    return visible;
}
fn optical_palette(t:f32,id:u32)->vec3f {
    let k=1.0-abs(fract(t*0.5)*2.0-1.0);
    if id==0u {return 0.55+0.45*cos(6.2831853*(vec3f(0.05,0.32,0.60)+t));}
    var a=vec3f(1);var b=vec3f(1);var c=vec3f(1);
    switch id {
        case 1u:{a=vec3f(0.3,0.002,0.008);b=vec3f(1,0.08,0.003);c=vec3f(1,0.8,0.12);}
        case 2u:{a=vec3f(0.008,0.035,0.35);b=vec3f(0,0.6,1);c=vec3f(0.7,1,1);}
        case 3u:{a=vec3f(0.2,0.005,0.6);b=vec3f(0,0.65,0.35);c=vec3f(0.4,1,0.05);}
        case 4u:{a=vec3f(0.05,0.3,1);b=vec3f(0.8,0.01,0.8);c=vec3f(1,0.16,0.28);}
        case 5u:{a=vec3f(0.3,0.045,0.005);b=vec3f(1,0.45,0.02);c=vec3f(1,0.9,0.45);}
        case 7u:{return mix(pow(f.orb_start.rgb,vec3f(2.2)),pow(f.orb_end.rgb,vec3f(2.2)),k);}
        default:{}
    }
    return mix(mix(a,b,clamp(k*2.0,0.0,1.0)),c,clamp(k*2.0-1.0,0.0,1.0));
}
fn optical_color(i:u32)->vec3f {
    let t=f32(i)/max(f.orbs.x-1.0,1.0)*f.orb_color.w+f.orb_color.z+f.clock.y*f.orb_tint.y;
    var color=optical_palette(t,u32(f.orb_color.y));
    if u32(f.orb_color.x)==0u {
        let k=f32(i)/max(f.orbs.x,1.0)*f.particle_color.z+f.look.w+f.clock.y*f.particle_light.z;
        // Match the particle palette, including its custom gradient.
        color=optical_palette(k,u32(f.particle_color.x));
        if u32(f.particle_color.x)==7u {
            let u=1.0-abs(fract(k*0.5)*2.0-1.0);
            color=mix(pow(f.color_start.rgb,vec3f(2.2)),pow(f.color_end.rgb,vec3f(2.2)),u);
        }
        let gray=dot(color,vec3f(0.2126,0.7152,0.0722));
        color=max(mix(vec3f(gray),color,f.particle_color.w),vec3f(0));
    }
    if u32(f.orb_color.x)==2u {color=pow(f.orb_start.rgb,vec3f(2.2));}
    if u32(f.orb_color.x)==3u {color=pow(f.orb_colors[min(i,4u)].rgb,vec3f(2.2));}
    let gray=dot(color,vec3f(0.2126,0.7152,0.0722));
    return max(mix(vec3f(gray),color,f.orb_tint.x),vec3f(0))*f.look.z*f.particle_light.w*f.orb_style.w;
}

fn optical_illumination(point:vec3f)->f32 {
    if f.orb_barrier.w<=0.0 {return 1.0;}
    var nearby=0.0;
    for(var j=0u;j<u32(f.orbs.x);j++) {
        let radius=optical_radius(j)*max(f.orb_style.z,1.0);
        let gap=max(length(point-optical_center(j))-radius,0.0);
        nearby=max(nearby,exp(-gap*gap/max(radius*radius*2.25,0.01)));
    }
    return 1.0+nearby*f.orb_barrier.w*2.0*(1.0+f.orb_flare.z);
}
