// One shader for every primitive of a morfo frame. Each instance is a quad
// covering one primitive; the fragment stage computes its coverage
// analytically (signed distances), so edges stay sharp at any scale.

struct Globals {
    size: vec2<f32>, // target size in logical pixels
    scale: f32,      // device pixels per logical pixel
    pad: f32,
}

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var<storage, read> points: array<vec2<f32>>;
// The image of the instances that show one (a 1×1 placeholder otherwise).
@group(1) @binding(0) var img: texture_2d<f32>;
@group(1) @binding(1) var img_samp: sampler;

struct Inst {
    @location(0) quad: vec4<f32>,   // area drawn: x, y, w, h
    @location(1) shape: vec4<f32>,  // rect: x, y, w, h | line: x0, y0, x1, y1 | polygon: start, len
    @location(2) fill: vec4<f32>,
    @location(3) stroke: vec4<f32>,
    @location(4) params: vec4<f32>, // rect: radius, stroke width | shadow: radius, blur | line: half width
    @location(5) uv: vec4<f32>,     // glyph: atlas u0, v0, u1, v1 | image: u0, v0, u1, v1
    @location(6) kind: u32,
}

struct Frag {
    @builtin(position) pos: vec4<f32>,
    @location(0) p: vec2<f32>,
    @location(1) @interpolate(flat) shape: vec4<f32>,
    @location(2) @interpolate(flat) fill: vec4<f32>,
    @location(3) @interpolate(flat) stroke: vec4<f32>,
    @location(4) @interpolate(flat) params: vec4<f32>,
    @location(5) t: vec2<f32>,
    @location(6) @interpolate(flat) kind: u32,
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, i: Inst) -> Frag {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    let p = i.quad.xy + c * i.quad.zw;
    var o: Frag;
    o.pos = vec4<f32>(p.x / g.size.x * 2.0 - 1.0, 1.0 - p.y / g.size.y * 2.0, 0.0, 1.0);
    o.p = p;
    o.shape = i.shape;
    o.fill = i.fill;
    o.stroke = i.stroke;
    o.params = i.params;
    o.t = mix(i.uv.xy, i.uv.zw, c);
    if i.kind == 6u {
        // Gradient: how far along it each corner is; linear, so the
        // interpolation in between is exact.
        let d = i.uv.zw - i.uv.xy;
        o.t = vec2<f32>(dot(p - i.uv.xy, d) / max(dot(d, d), 1e-6), 0.0);
    }
    o.kind = i.kind;
    return o;
}

// Signed distance to a rounded rectangle (negative inside).
fn sd_rrect(p: vec2<f32>, r: vec4<f32>, radius: f32) -> f32 {
    let h = r.zw * 0.5;
    let rad = clamp(radius, 0.0, min(h.x, h.y));
    let q = abs(p - (r.xy + h)) - (h - vec2<f32>(rad));
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rad;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

// Coverage of a pixel whose center is `d` logical pixels from an edge.
fn edge(d: f32) -> f32 {
    return clamp(0.5 - d * g.scale, 0.0, 1.0);
}

fn cross2(a: vec2<f32>, b: vec2<f32>) -> f32 {
    return a.x * b.y - a.y * b.x;
}

@fragment
fn fs(o: Frag) -> @location(0) vec4<f32> {
    // Premultiplied output.
    var out = vec4<f32>(0.0);
    switch o.kind {
        case 0u: { // rounded rectangle, filled and optionally stroked
            let d = sd_rrect(o.p, o.shape, o.params.x);
            let fa = o.fill.a * edge(d);
            var sa = 0.0;
            if o.params.y > 0.0 {
                let w = o.params.y;
                let inner = vec4<f32>(o.shape.xy + vec2<f32>(w), o.shape.zw - vec2<f32>(2.0 * w));
                var inner_cov = 0.0;
                if inner.z > 0.0 && inner.w > 0.0 {
                    inner_cov = edge(sd_rrect(o.p, inner, o.params.x - w));
                }
                sa = o.stroke.a * max(edge(d) - inner_cov, 0.0);
            }
            out = vec4<f32>(o.stroke.rgb * sa + o.fill.rgb * fa * (1.0 - sa), sa + fa * (1.0 - sa));
        }
        case 1u: { // soft shadow
            let d = sd_rrect(o.p, o.shape, o.params.x);
            let t = 1.0 - clamp(d / max(o.params.y, 0.01) + 0.35, 0.0, 1.0);
            let a = o.fill.a * t * t;
            out = vec4<f32>(o.fill.rgb * a, a);
        }
        case 2u: { // stroke with round ends
            let a = o.fill.a * edge(sd_segment(o.p, o.shape.xy, o.shape.zw) - o.params.x);
            out = vec4<f32>(o.fill.rgb * a, a);
        }
        case 3u: { // glyph from the atlas
            let a = o.fill.a * textureSampleLevel(atlas, samp, o.t, 0.0).r;
            out = vec4<f32>(o.fill.rgb * a, a);
        }
        case 5u: { // image with rounded corners, tinted by `fill`
            let c = textureSampleLevel(img, img_samp, o.t, 0.0);
            let k = o.fill.a * edge(sd_rrect(o.p, o.shape, o.params.x));
            out = vec4<f32>(c.rgb * o.fill.rgb, 1.0) * c.a * k;
        }
        case 6u: { // rounded rectangle filled with a linear gradient
            let c = mix(o.fill, o.stroke, clamp(o.t.x, 0.0, 1.0));
            let a = c.a * edge(sd_rrect(o.p, o.shape, o.params.x));
            out = vec4<f32>(c.rgb * a, a);
        }
        default: { // filled polygon, non-zero winding
            let start = u32(o.shape.x);
            let n = u32(o.shape.y);
            var wind = 0;
            var dmin = 1e9;
            for (var k = 0u; k < n; k = k + 1u) {
                let a = points[start + k];
                let b = points[start + (k + 1u) % n];
                // A seam joining two contours of one shape is drawn as a
                // repeated point, the seam, and a repeated point: it is no
                // edge, and must not soften what is around it.
                let before = points[start + (k + n - 1u) % n];
                let after = points[start + (k + 2u) % n];
                let seam = all(before == a) && all(after == b) && any(a != b);
                if !seam {
                    dmin = min(dmin, sd_segment(o.p, a, b));
                }
                let side = cross2(b - a, o.p - a);
                if a.y <= o.p.y {
                    if b.y > o.p.y && side > 0.0 {
                        wind = wind + 1;
                    }
                } else if b.y <= o.p.y && side < 0.0 {
                    wind = wind - 1;
                }
            }
            let sd = select(dmin, -dmin, wind != 0);
            let a = o.fill.a * edge(sd);
            out = vec4<f32>(o.fill.rgb * a, a);
        }
    }
    return out;
}
