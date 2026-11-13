//! Coherent native samples of one immutable material scene.
use crate::{Lighting, Maps, Scene, Settings};
pub use craft_stereo_camera::Budget;
use craft_stereo_camera::{Camera, CameraPair, DepthRange, ProjectionKind};
use materialcraft_geom::glam::Vec3;
use std::sync::atomic::{AtomicBool, Ordering};

/// One complete left-first side-by-side raster, never a partial eye update.
/// Samples are finite supersampling, not a cross-frame progressive history.
pub struct Frame {
    pub pixels: Vec<u8>,
    pub camera: CameraPair,
    pub samples_per_eye: [u32; 2],
    pub primary_rays: [usize; 2],
    pub key_light: Vec3,
}

/// Retain the preview's units, target, 35-degree projection and displaced geometry.
pub fn camera_pair(scene: &Scene, settings: &Settings, width: u32, height: u32, budget: Budget) -> Result<CameraPair, String> {
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
        return Err("stereo render dimensions must be between 1 and 4096 per eye".into());
    }
    let cam = crate::preview::camera(scene, settings);
    let eye = cam.eye();
    if !eye.is_finite()
        || !cam.target.is_finite()
        || !cam.scene_radius.is_finite()
        || cam.scene_radius <= 0.0
        || !cam.distance.is_finite()
        || cam.distance <= 0.0
        || (cam.target - eye).try_normalize().is_none_or(|d| d.cross(Vec3::Y).try_normalize().is_none())
        || scene.mesh.positions.iter().any(|p| !p.is_finite())
    {
        return Err("the material scene or central camera is not finite".into());
    }
    let r = cam.scene_radius.max(1e-3);
    let near = (cam.distance - r * 4.0).max(r * 0.01).max(1e-4);
    let far = cam.distance + r * 50.0;
    if !far.is_finite() || near >= far {
        return Err("the material camera has no finite clipping range".into());
    }
    let view = cam.view();
    let (lo, hi) = scene.mesh.bounds();
    let depths = DepthRange::from_aabb(view, lo, hi, near).map_err(|e| e.to_string())?;
    let projection = cam.proj(width as f32 / height as f32);
    Camera {
        eye,
        view,
        projection,
        kind: ProjectionKind::Perspective,
        infinity_projection_x: projection.x_axis.x,
        zero_depth: cam.distance,
        depths,
        width,
        budget,
    }
    .pair()
    .map_err(|e| e.to_string())
}

/// Borrow geometry/BVH, material maps and lighting once for both eyes.
/// The central key light is calculated once by the ordinary sample renderer.
/// All pixel accumulators are local to their eye; there is no temporal state to share.
pub fn render_pair_cancellable(
    scene: &Scene,
    maps: &Maps,
    light: &Lighting,
    settings: &Settings,
    width: u32,
    height: u32,
    budget: Budget,
    cancel: &AtomicBool,
) -> Result<Option<Frame>, String> {
    let camera = camera_pair(scene, settings, width, height, budget)?;
    let rays = if camera.baseline == 0.0 {
        None
    } else {
        Some([camera.eyes[0].rays().map_err(|e| e.to_string())?, camera.eyes[1].rays().map_err(|e| e.to_string())?])
    };
    let Some(rendered) = crate::preview::render_samples(scene, maps, light, settings, width, height, cancel, rays, 2) else {
        return if cancel.load(Ordering::Relaxed) { Ok(None) } else { Err("could not render a complete material stereo pair".into()) };
    };
    let spp = settings.supersample.clamp(1, 4).pow(2);
    let expected = width as usize * height as usize * spp as usize;
    if rendered.primary_rays != [expected; 2] {
        return Err("the material eyes did not finish the same sample count".into());
    }
    Ok(Some(Frame { pixels: rendered.pixels, camera, samples_per_eye: [spp; 2], primary_rays: rendered.primary_rays, key_light: rendered.key_light }))
}
