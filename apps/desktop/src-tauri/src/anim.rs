//! Keyframe animation — Blender-grade F-curves for 2D motion design.
//!
//! A `Track` holds keyframes (frame → value) for one named property. At a given
//! frame we find the bracketing keyframes and interpolate using the outgoing
//! key's easing (cubic bezier, linear, or step).
//!
//! Conventions:
//!  - Easing belongs to the *outgoing* keyframe (the key on the left of the gap).
//!  - Frames are f32 (so we can sample sub-frame for retiming experiments).
//!  - Colors interpolate in OKLCH, not RGB (RGB lerps look muddy through gray).

use serde::{Deserialize, Serialize};

// ── Easing ────────────────────────────────────────────────────────────────────

/// Cubic-bezier control points for an easing curve. `p1` and `p2` are the two
/// inner handles of a curve from (0,0) → (1,1) — same model as Blender's
/// F-curve bezier handles and CSS `cubic-bezier()`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Easing {
    Linear,
    /// Hold the outgoing value until the next keyframe (no interpolation).
    Step,
    Bezier {
        p1: [f32; 2],
        p2: [f32; 2],
    },
}

impl Default for Easing {
    fn default() -> Self {
        // CSS `ease-in-out`.
        Easing::Bezier { p1: [0.42, 0.0], p2: [0.58, 1.0] }
    }
}

impl Easing {
    pub fn linear() -> Self { Easing::Linear }
    pub fn step() -> Self { Easing::Step }
    pub fn ease_in() -> Self { Easing::Bezier { p1: [0.42, 0.0], p2: [1.0, 1.0] } }
    pub fn ease_out() -> Self { Easing::Bezier { p1: [0.0, 0.0], p2: [0.58, 1.0] } }
    pub fn ease_in_out() -> Self { Easing::Bezier { p1: [0.42, 0.0], p2: [0.58, 1.0] } }
    /// "back" overshoot — like `cubic-bezier(0.68, -0.55, 0.27, 1.55)`.
    pub fn ease_back() -> Self { Easing::Bezier { p1: [0.68, -0.55], p2: [0.27, 1.55] } }

    /// Map progress 0..1 through the easing curve. Returns y at the curve point
    /// whose x equals `t`. May overshoot 0..1 for back/elastic-style handles.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::Step => 0.0,
            Easing::Bezier { p1, p2 } => bezier_y_for_x(p1[0], p1[1], p2[0], p2[1], t),
        }
    }
}

/// Cubic bezier y-for-x. Anchors are fixed at (0,0) and (1,1); the two inner
/// handles are (x1,y1) and (x2,y2). Newton-Raphson on the parametric form,
/// fallback to bisection when the derivative is tiny.
fn bezier_y_for_x(x1: f32, y1: f32, x2: f32, y2: f32, x_target: f32) -> f32 {
    if x_target <= 0.0 { return 0.0; }
    if x_target >= 1.0 { return 1.0; }

    // x(s) = 3(1-s)²·s·x1 + 3(1-s)·s²·x2 + s³
    // y(s) = same with y1/y2.
    let curve_x = |s: f32| {
        let one = 1.0 - s;
        3.0 * one * one * s * x1 + 3.0 * one * s * s * x2 + s * s * s
    };
    let curve_y = |s: f32| {
        let one = 1.0 - s;
        3.0 * one * one * s * y1 + 3.0 * one * s * s * y2 + s * s * s
    };
    let dx_ds = |s: f32| {
        let one = 1.0 - s;
        3.0 * one * one * x1 + 6.0 * one * s * (x2 - x1) + 3.0 * s * s * (1.0 - x2)
    };

    // Initial guess: s ≈ x_target (works well when curve is near linear).
    let mut s = x_target;
    for _ in 0..8 {
        let cx = curve_x(s) - x_target;
        if cx.abs() < 1e-5 { return curve_y(s); }
        let d = dx_ds(s);
        if d.abs() < 1e-6 { break; }
        s -= cx / d;
        s = s.clamp(0.0, 1.0);
    }
    // Fallback: bisection.
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    for _ in 0..32 {
        s = 0.5 * (lo + hi);
        let cx = curve_x(s);
        if cx < x_target { lo = s; } else { hi = s; }
        if (hi - lo).abs() < 1e-5 { break; }
    }
    curve_y(s)
}

// ── KeyValue ──────────────────────────────────────────────────────────────────

/// A keyframe value. The variant must match across both keys in an interpolation
/// pair (mismatches fall back to step-hold of the outgoing value).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyValue {
    Scalar(f32),
    Vec3([f32; 3]),
    /// rgba 0..1.
    Color([f32; 4]),
    /// Step-only (no interpolation between strings).
    Str(String),
}

impl KeyValue {
    pub fn as_scalar(&self) -> Option<f32> {
        match self { KeyValue::Scalar(s) => Some(*s), _ => None }
    }
    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        match self { KeyValue::Vec3(v) => Some(*v), _ => None }
    }
    pub fn as_color(&self) -> Option<[f32; 4]> {
        match self { KeyValue::Color(c) => Some(*c), _ => None }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { KeyValue::Str(s) => Some(s.as_str()), _ => None }
    }
}

// ── Track ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    pub frame: f32,
    pub value: KeyValue,
    #[serde(default)]
    pub easing: Easing,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Track {
    pub keys: Vec<Keyframe>,
}

impl Track {
    /// Insert-or-replace a keyframe at `frame`. Keeps keys sorted ascending.
    pub fn insert(&mut self, frame: f32, value: KeyValue, easing: Easing) {
        if let Some(k) = self.keys.iter_mut().find(|k| (k.frame - frame).abs() < 0.001) {
            k.value = value;
            k.easing = easing;
        } else {
            self.keys.push(Keyframe { frame, value, easing });
            self.keys.sort_by(|a, b| a.frame.partial_cmp(&b.frame).unwrap());
        }
    }

    /// Remove the keyframe at `frame` (if any). Returns true if removed.
    pub fn remove_at(&mut self, frame: f32) -> bool {
        let before = self.keys.len();
        self.keys.retain(|k| (k.frame - frame).abs() >= 0.001);
        self.keys.len() != before
    }

    pub fn is_empty(&self) -> bool { self.keys.is_empty() }

    /// Sample the track at `frame`. Returns None if the track has no keys.
    pub fn sample(&self, frame: f32) -> Option<KeyValue> {
        match self.keys.len() {
            0 => None,
            1 => Some(self.keys[0].value.clone()),
            _ => {
                if frame <= self.keys[0].frame { return Some(self.keys[0].value.clone()); }
                let last = self.keys.last().unwrap();
                if frame >= last.frame { return Some(last.value.clone()); }
                for w in self.keys.windows(2) {
                    let (a, b) = (&w[0], &w[1]);
                    if frame >= a.frame && frame <= b.frame {
                        let span = (b.frame - a.frame).max(0.0001);
                        let raw = (frame - a.frame) / span;
                        let t = match a.easing {
                            Easing::Step => 0.0,
                            _ => a.easing.apply(raw),
                        };
                        return Some(lerp_value(&a.value, &b.value, t));
                    }
                }
                Some(last.value.clone())
            }
        }
    }
}

fn lerp_value(a: &KeyValue, b: &KeyValue, t: f32) -> KeyValue {
    match (a, b) {
        (KeyValue::Scalar(x), KeyValue::Scalar(y)) => KeyValue::Scalar(x + (y - x) * t),
        (KeyValue::Vec3(x), KeyValue::Vec3(y)) => KeyValue::Vec3([
            x[0] + (y[0] - x[0]) * t,
            x[1] + (y[1] - x[1]) * t,
            x[2] + (y[2] - x[2]) * t,
        ]),
        (KeyValue::Color(x), KeyValue::Color(y)) => KeyValue::Color(lerp_color_oklch(*x, *y, t)),
        // Strings (and mismatched variants) hold the outgoing value.
        _ => a.clone(),
    }
}

// ── Color interpolation (OKLCH) ──────────────────────────────────────────────
//
// OKLCH is OKLab in cylindrical form (L, C, h). Lerping in OKLCH avoids the
// muddy-gray midpoints of RGB lerps and keeps perceptual lightness even.
// sRGB → linear → OKLab → OKLCH → lerp → OKLab → linear → sRGB.

fn lerp_color_oklch(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let (la, ca, ha) = srgb_to_oklch([a[0], a[1], a[2]]);
    let (lb, cb, hb) = srgb_to_oklch([b[0], b[1], b[2]]);
    let l = la + (lb - la) * t;
    let c = ca + (cb - ca) * t;
    // Shortest-arc hue interpolation.
    let mut dh = hb - ha;
    if dh > 180.0 { dh -= 360.0; }
    if dh < -180.0 { dh += 360.0; }
    let h = ha + dh * t;
    let rgb = oklch_to_srgb(l, c, h);
    let alpha = a[3] + (b[3] - a[3]) * t;
    [rgb[0], rgb[1], rgb[2], alpha]
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

fn srgb_to_oklch(rgb: [f32; 3]) -> (f32, f32, f32) {
    let r = srgb_to_linear(rgb[0]);
    let g = srgb_to_linear(rgb[1]);
    let b = srgb_to_linear(rgb[2]);
    // Linear sRGB → OKLab (Ottosson 2020).
    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;
    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();
    let lab_l = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_;
    let lab_a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_;
    let lab_b = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_;
    let c = (lab_a * lab_a + lab_b * lab_b).sqrt();
    let mut h = lab_b.atan2(lab_a).to_degrees();
    if h < 0.0 { h += 360.0; }
    (lab_l, c, h)
}

fn oklch_to_srgb(l: f32, c: f32, h: f32) -> [f32; 3] {
    let a = c * h.to_radians().cos();
    let b = c * h.to_radians().sin();
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let l_lin = l_ * l_ * l_;
    let m_lin = m_ * m_ * m_;
    let s_lin = s_ * s_ * s_;
    let r =  4.0767416621 * l_lin - 3.3077115913 * m_lin + 0.2309699292 * s_lin;
    let g = -1.2684380046 * l_lin + 2.6097574011 * m_lin - 0.3413193965 * s_lin;
    let b =  -0.0041960863 * l_lin - 0.7034186147 * m_lin + 1.7076147010 * s_lin;
    [
        linear_to_srgb(r.clamp(0.0, 1.0)),
        linear_to_srgb(g.clamp(0.0, 1.0)),
        linear_to_srgb(b.clamp(0.0, 1.0)),
    ]
}

// ── Color string helpers (used by scene.rs → render state) ────────────────────

/// Parse "#rgb", "#rrggbb", or "#rrggbbaa" → rgba 0..1.
pub fn parse_css_color(s: &str) -> Option<[f32; 4]> {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        3 => {
            let r = u8::from_str_radix(&s[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&s[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&s[2..3].repeat(2), 16).ok()?;
            Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
        }
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0])
        }
        _ => None,
    }
}

pub fn rgba_to_css(c: [f32; 4]) -> String {
    let r = (c[0] * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (c[1] * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (c[2] * 255.0).round().clamp(0.0, 255.0) as u8;
    if c[3] >= 0.999 {
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    } else {
        format!("rgba({},{},{},{:.3})", r, g, b, c[3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_endpoints() {
        let e = Easing::ease_in_out();
        assert!((e.apply(0.0) - 0.0).abs() < 1e-4);
        assert!((e.apply(1.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn bezier_overshoot() {
        // back-out: at t≈0.7, y should overshoot 1.0
        let e = Easing::ease_back();
        let y = e.apply(0.7);
        assert!(y > 1.0 || y < 0.0, "back curve should overshoot or undershoot, got {y}");
    }

    #[test]
    fn track_sample_basic() {
        let mut t = Track::default();
        t.insert(0.0, KeyValue::Scalar(0.0), Easing::Linear);
        t.insert(10.0, KeyValue::Scalar(100.0), Easing::Linear);
        assert!((t.sample(5.0).unwrap().as_scalar().unwrap() - 50.0).abs() < 0.1);
    }

    #[test]
    fn color_lerp_midpoint_not_gray() {
        let red = [1.0, 0.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 1.0];
        let mid = lerp_color_oklch(red, blue, 0.5);
        // RGB lerp would give (0.5, 0, 0.5) — a muddy purple. OKLCH lerp should
        // produce something more vivid; just check it's not the trivial RGB lerp.
        let trivial = [0.5_f32, 0.0, 0.5];
        let diff = (mid[0] - trivial[0]).abs() + (mid[1] - trivial[1]).abs() + (mid[2] - trivial[2]).abs();
        assert!(diff > 0.05, "OKLCH lerp produced near-RGB-lerp output: {:?}", mid);
    }

    #[test]
    fn parse_color() {
        assert_eq!(parse_css_color("#ff8000"), Some([1.0, 128.0/255.0, 0.0, 1.0]));
        assert_eq!(parse_css_color("#f80"), Some([1.0, 136.0/255.0, 0.0, 1.0]));
    }
}
