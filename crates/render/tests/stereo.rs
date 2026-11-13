use materialcraft_geom::{
    Mesh,
    glam::{Vec2, Vec3},
};
use materialcraft_render::{
    Lighting, Maps, Scene, Settings,
    stereo::{Budget, render_pair_cancellable},
};
use std::sync::atomic::AtomicBool;

fn scene() -> Scene {
    let mut mesh = Mesh::default();
    for (z, y) in [(1.0, 0.4), (-1.0, -0.4)] {
        let base = mesh.positions.len() as u32;
        for (x, dy) in [(-0.4, -0.2), (0.4, -0.2), (0.4, 0.2), (-0.4, 0.2)] {
            mesh.positions.push(Vec3::new(x, y + dy, z));
            mesh.uvs.push(Vec2::ZERO);
        }
        mesh.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Scene::new(mesh)
}

#[test]
fn two_depths_use_one_scene_and_equal_finite_sample_counts() {
    let scene = scene();
    let maps = Maps::default();
    let light = Lighting::new("studio");
    let settings = Settings { yaw: 0.0, pitch: 0.0, distance: 5.0 / scene.radius, supersample: 2, alpha_view: true, ..Settings::default() };
    let pair =
        render_pair_cancellable(&scene, &maps, &light, &settings, 320, 180, Budget { sunk: 24.0, popped: 12.0 }, &AtomicBool::new(false)).unwrap().unwrap();
    assert_eq!(pair.samples_per_eye, [4, 4]);
    assert_eq!(pair.primary_rays, [320 * 180 * 4; 2]);
    assert!((pair.key_light - Vec3::new(-0.6, 0.75, 0.5).normalize()).length() < 0.00001);
    assert_eq!(pair.pixels.len(), 640 * 180 * 4);
    assert!(pair.camera.baseline > 0.0);
    // At D=5 the infinity limit fixes e*f=120: d(4)=+6, d(6)=-4.
    for (depth, yrange, expected) in [(4.0, 15..85, 6.0), (6.0, 95..165, -4.0)] {
        let mut centers = Vec::new();
        for eye in 0..2 {
            let mut sum = 0.0;
            let mut count = 0u32;
            for y in yrange.clone() {
                for x in 0..320 {
                    let at = (y * 640 + eye * 320 + x) * 4;
                    if pair.pixels[at..at + 3] == [255, 255, 255] {
                        sum += x as f64;
                        count += 1;
                    }
                }
            }
            assert!(count > 100, "the depth-{depth} surface is visible");
            centers.push(sum / f64::from(count));
        }
        assert!((centers[0] - centers[1] - expected).abs() < 0.05, "depth {depth}: {centers:?}");
    }
}

#[test]
fn cancellation_and_invalid_dimensions_never_publish_a_half_pair() {
    let scene = scene();
    let light = Lighting::new("studio");
    let maps = Maps::default();
    let settings = Settings::default();
    let budget = Budget { sunk: 0.0, popped: 0.0 };
    assert!(render_pair_cancellable(&scene, &maps, &light, &settings, 32, 24, budget, &AtomicBool::new(true)).unwrap().is_none());
    for [w, h] in [[0, 24], [32, 0], [4097, 24], [32, u32::MAX]] {
        assert!(render_pair_cancellable(&scene, &maps, &light, &settings, w, h, budget, &AtomicBool::new(false)).is_err());
    }
}

#[test]
fn actual_sample_counts_and_scene_are_stable_at_every_supersampling_level() {
    let scene = scene();
    let maps = Maps::default();
    let light = Lighting::new("studio");
    let budget = Budget { sunk: 3.0, popped: 1.5 };
    for ss in 1u32..=4 {
        let settings = Settings { yaw: 0.0, pitch: 0.0, distance: 5.0 / scene.radius, supersample: ss, key_light: 1.0, ao_rays: 2, ..Settings::default() };
        let first = render_pair_cancellable(&scene, &maps, &light, &settings, 32, 24, budget, &AtomicBool::new(false)).unwrap().unwrap();
        let second = render_pair_cancellable(&scene, &maps, &light, &settings, 32, 24, budget, &AtomicBool::new(false)).unwrap().unwrap();
        assert_eq!(first.primary_rays, [32 * 24 * (ss * ss) as usize; 2]);
        assert_eq!(first.samples_per_eye, [ss * ss; 2]);
        assert_eq!(first.pixels, second.pixels, "neither eye retains history from another frame");
        assert_eq!(first.key_light, second.key_light);
        assert_eq!(first.camera, second.camera);
    }
}
