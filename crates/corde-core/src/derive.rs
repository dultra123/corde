//! Physics helpers shared by adapters. Pure functions, unit-tested.

/// Kelvin → Celsius.
#[inline]
pub fn k_to_c(k: f64) -> f32 {
    (k - 273.15) as f32
}

/// Radians → degrees.
#[inline]
pub fn rad_to_deg(r: f64) -> f32 {
    r.to_degrees() as f32
}

/// m/s → km/h.
#[inline]
pub fn ms_to_kmh(v: f32) -> f32 {
    v * 3.6
}

/// Longitudinal slip ratio from the wheel's longitudinal patch velocity and the
/// ground velocity under it. Positive = wheel spinning faster than the ground
/// (traction loss), negative = wheel slower (locking). Clamped to ±1.
///
/// In the rFactor family, `longitudinal_patch_vel` is the contact-patch
/// velocity relative to the ground, so slip ratio ≈ patch / |ground|.
#[inline]
pub fn slip_ratio(longitudinal_patch_vel: f64, longitudinal_ground_vel: f64) -> f32 {
    let g = longitudinal_ground_vel.abs();
    if g < 0.5 {
        return 0.0;
    }
    (longitudinal_patch_vel / g).clamp(-1.0, 1.0) as f32
}

/// Slip angle in degrees from lateral and longitudinal ground velocities at the
/// contact patch.
#[inline]
pub fn slip_angle_deg(lateral_ground_vel: f64, longitudinal_ground_vel: f64) -> f32 {
    let g = longitudinal_ground_vel.abs();
    if g < 0.5 {
        return 0.0;
    }
    (lateral_ground_vel.atan2(g)).to_degrees() as f32
}

/// Acceleration (m/s²) → g.
#[inline]
pub fn to_g(a: f64) -> f32 {
    (a / 9.80665) as f32
}

/// Heading angle (radians) from the third row of the rFactor orientation
/// matrix (`mOri[2]`), i.e. the world-space direction of the local +z axis
/// (which points to the *back* of the car). We flip it to get the forward
/// direction.
#[inline]
pub fn heading_from_ori_row2(ori_z: [f64; 3]) -> f32 {
    // forward = -ori_z ; heading measured in the X/Z ground plane.
    (-ori_z[0]).atan2(-ori_z[2]) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert!((k_to_c(373.15) - 100.0).abs() < 1e-4);
        assert!((rad_to_deg(std::f64::consts::PI) - 180.0).abs() < 1e-4);
        assert!((ms_to_kmh(10.0) - 36.0).abs() < 1e-5);
    }

    #[test]
    fn slip() {
        assert_eq!(slip_ratio(0.0, 0.1), 0.0);
        assert!((slip_ratio(5.0, 50.0) - 0.1).abs() < 1e-6);
        assert_eq!(slip_ratio(-100.0, 50.0), -1.0);
        assert!((slip_angle_deg(10.0, 10.0) - 45.0).abs() < 1e-4);
    }
}
