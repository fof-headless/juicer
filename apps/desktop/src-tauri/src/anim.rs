//! Keyframe animation — the native equivalent of Blender's F-curves.
//!
//! A `Track` holds keyframes (time → value) for one property. Values can be
//! scalar (opacity) or vec3 (position/rotation/scale). At a given time we find
//! the bracketing keyframes and interpolate with the chosen easing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Step, // hold previous value until next key
}

impl Default for Easing {
    fn default() -> Self {
        Easing::EaseInOut
    }
}

impl Easing {
    /// Remap a normalized 0..1 progress through the easing curve.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::Step => 0.0,
        }
    }
}

/// A keyframe value: either a scalar or a 3-component vector.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyValue {
    Scalar(f32),
    Vec3([f32; 3]),
}

impl KeyValue {
    pub fn as_vec3(&self) -> [f32; 3] {
        match self {
            KeyValue::Vec3(v) => *v,
            KeyValue::Scalar(s) => [*s, *s, *s],
        }
    }
    pub fn as_scalar(&self) -> f32 {
        match self {
            KeyValue::Scalar(s) => *s,
            KeyValue::Vec3(v) => v[0],
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Keyframe {
    /// Frame number (integer timeline position).
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
    /// Insert or replace a keyframe at `frame`. Keeps keys sorted by frame.
    pub fn insert(&mut self, frame: f32, value: KeyValue, easing: Easing) {
        if let Some(k) = self.keys.iter_mut().find(|k| (k.frame - frame).abs() < 0.001) {
            k.value = value;
            k.easing = easing;
        } else {
            self.keys.push(Keyframe { frame, value, easing });
            self.keys.sort_by(|a, b| a.frame.partial_cmp(&b.frame).unwrap());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Evaluate the track at `frame`, returning None if no keys exist.
    pub fn sample(&self, frame: f32) -> Option<KeyValue> {
        match self.keys.len() {
            0 => None,
            1 => Some(self.keys[0].value),
            _ => {
                // Before first key → first value; after last → last value.
                if frame <= self.keys[0].frame {
                    return Some(self.keys[0].value);
                }
                if frame >= self.keys[self.keys.len() - 1].frame {
                    return Some(self.keys[self.keys.len() - 1].value);
                }
                // Find bracketing pair.
                for w in self.keys.windows(2) {
                    let (a, b) = (&w[0], &w[1]);
                    if frame >= a.frame && frame <= b.frame {
                        let span = (b.frame - a.frame).max(0.0001);
                        let raw = (frame - a.frame) / span;
                        // Easing belongs to the outgoing key (a).
                        let t = if a.easing == Easing::Step {
                            0.0
                        } else {
                            a.easing.apply(raw)
                        };
                        return Some(lerp_value(a.value, b.value, t));
                    }
                }
                Some(self.keys[self.keys.len() - 1].value)
            }
        }
    }
}

fn lerp_value(a: KeyValue, b: KeyValue, t: f32) -> KeyValue {
    match (a, b) {
        (KeyValue::Scalar(x), KeyValue::Scalar(y)) => KeyValue::Scalar(x + (y - x) * t),
        _ => {
            let av = a.as_vec3();
            let bv = b.as_vec3();
            KeyValue::Vec3([
                av[0] + (bv[0] - av[0]) * t,
                av[1] + (bv[1] - av[1]) * t,
                av[2] + (bv[2] - av[2]) * t,
            ])
        }
    }
}
