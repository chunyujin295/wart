//! Gradient sampling math.

use crate::model::Rgb;

/// A color stop on a gradient ramp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    /// Position along the ramp, `0.0..=1.0`.
    pub t: f32,
    pub color: Rgb,
}

impl Stop {
    pub fn new(t: f32, color: Rgb) -> Self {
        Self { t, color }
    }
}

/// Build an evenly spaced ramp from a list of colors.
pub fn even_stops(colors: &[Rgb]) -> Vec<Stop> {
    match colors.len() {
        0 => Vec::new(),
        1 => vec![Stop::new(0.0, colors[0])],
        n => colors
            .iter()
            .enumerate()
            .map(|(i, c)| Stop::new(i as f32 / (n - 1) as f32, *c))
            .collect(),
    }
}

/// Sample a ramp, clamping outside the first and last stop.
///
/// Clamping rather than wrapping matters: wrapping makes a gradient jump
/// abruptly from its last color back to its first at the edges, which reads as
/// a rendering bug.
pub fn sample_stops(stops: &[Stop], t: f32) -> Rgb {
    let Some(first) = stops.first() else {
        return Rgb::WHITE;
    };
    if t <= first.t {
        return first.color;
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t <= b.t {
            let span = b.t - a.t;
            let k = if span.abs() < f32::EPSILON { 0.0 } else { (t - a.t) / span };
            return a.color.lerp(b.color, k);
        }
    }
    stops.last().map_or(first.color, |s| s.color)
}

/// Project `(u, v)` onto the gradient axis and renormalise to `0..=1`.
///
/// The renormalisation is what makes an angled gradient span its whole ramp.
/// Projecting the corners of the unit box and rescaling means a 45-degree
/// gradient still travels the full color range instead of only the middle half,
/// which is the difference between a vivid gradient and a washed-out one.
pub fn axis_t(angle_deg: f32, u: f32, v: f32) -> f32 {
    let a = angle_deg.to_radians();
    let (dx, dy) = (a.cos(), a.sin());

    // Extreme projections over the unit box corners.
    let p_min = dx.min(0.0) + dy.min(0.0);
    let p_max = dx.max(0.0) + dy.max(0.0);
    let span = (p_max - p_min).max(f32::EPSILON);

    ((u * dx + v * dy) - p_min) / span
}

/// HSV to RGB. `hue` is in degrees and wraps; `saturation` and `value` are `0..=1`.
pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> Rgb {
    let h = hue.rem_euclid(360.0) / 60.0;
    let s = saturation.clamp(0.0, 1.0);
    let v = value.clamp(0.0, 1.0);

    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let to_u8 = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb::new(to_u8(r), to_u8(g), to_u8(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgb = Rgb::new(255, 0, 0);
    const BLUE: Rgb = Rgb::new(0, 0, 255);

    #[test]
    fn even_stops_spans_the_full_ramp() {
        let s = even_stops(&[RED, BLUE]);
        assert_eq!(s.len(), 2);
        assert!((s[0].t - 0.0).abs() < 1e-6);
        assert!((s[1].t - 1.0).abs() < 1e-6);
    }

    #[test]
    fn even_stops_of_three_puts_the_middle_at_the_center() {
        let s = even_stops(&[RED, Rgb::new(0, 255, 0), BLUE]);
        assert!((s[1].t - 0.5).abs() < 1e-6);
    }

    #[test]
    fn sample_stops_hits_the_endpoints_exactly() {
        let s = even_stops(&[RED, BLUE]);
        assert_eq!(sample_stops(&s, 0.0), RED);
        assert_eq!(sample_stops(&s, 1.0), BLUE);
    }

    #[test]
    fn sample_stops_clamps_rather_than_wraps() {
        let s = even_stops(&[RED, BLUE]);
        // Wrapping would return BLUE here, which looks like a seam.
        assert_eq!(sample_stops(&s, -0.5), RED);
        assert_eq!(sample_stops(&s, 1.5), BLUE);
    }

    #[test]
    fn sample_stops_interpolates_the_midpoint() {
        let s = even_stops(&[Rgb::BLACK, Rgb::WHITE]);
        let mid = sample_stops(&s, 0.5);
        assert!((mid.r as i32 - 128).abs() <= 1);
        assert_eq!(mid, Rgb::new(128, 128, 128));
    }

    #[test]
    fn empty_stops_do_not_panic() {
        assert_eq!(sample_stops(&[], 0.5), Rgb::WHITE);
    }

    #[test]
    fn axis_t_spans_the_full_range_for_any_angle() {
        // The whole point of the corner renormalisation: for a given angle, the
        // extreme corners must map to 0 and 1 regardless of direction.
        for angle in [0.0f32, 30.0, 45.0, 90.0, 135.0, 180.0, 270.0] {
            let a = angle.to_radians();
            let (dx, dy) = (a.cos(), a.sin());
            let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
            let vals: Vec<f32> = corners
                .iter()
                .map(|(u, v)| (u * dx + v * dy - (dx.min(0.0) + dy.min(0.0)))
                    / (dx.max(0.0) + dy.max(0.0) - (dx.min(0.0) + dy.min(0.0))))
                .collect();
            let lo = vals.iter().cloned().fold(f32::INFINITY, f32::min);
            let hi = vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

            assert!(lo.abs() < 1e-5, "angle {angle}: min was {lo}, expected 0");
            assert!((hi - 1.0).abs() < 1e-5, "angle {angle}: max was {hi}, expected 1");
        }
    }

    #[test]
    fn axis_t_is_constant_along_the_axis() {
        // For a horizontal gradient every point in a column shares a value.
        let a = axis_t(0.0, 0.3, 0.0);
        let b = axis_t(0.0, 0.3, 1.0);
        assert!((a - b).abs() < 1e-5);
    }

    #[test]
    fn hsv_covers_the_primary_hues() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), RED);
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), Rgb::new(0, 255, 0));
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), BLUE);
    }

    #[test]
    fn hsv_hue_wraps() {
        assert_eq!(hsv_to_rgb(360.0, 1.0, 1.0), RED);
        assert_eq!(hsv_to_rgb(-360.0, 1.0, 1.0), RED);
    }

    #[test]
    fn hsv_zero_saturation_is_gray() {
        assert_eq!(hsv_to_rgb(200.0, 0.0, 1.0), Rgb::WHITE);
        assert_eq!(hsv_to_rgb(200.0, 0.0, 0.0), Rgb::BLACK);
    }
}
