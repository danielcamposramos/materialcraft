//! A read-only companion rendering the ordinary preview's immutable scene snapshot.
use crate::{
    MaterialCraftApp,
    i18n::{tr, trf},
};
use craft_stereo_window::{Picture, StereoWindow};
use materialcraft_render::stereo::Budget;
use serde::Serialize;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, TryRecvError},
};
use std::time::{Duration, Instant};

type Key = (u64, [u32; 2], [u32; 2]);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub state: String,
    pub size: [u32; 2],
    pub budget: [f32; 2],
    pub samples_per_eye: [u32; 2],
    pub primary_rays: [usize; 2],
    pub baseline: f32,
    pub key_light_direction: [f32; 3],
    pub key_light_strength: f32,
}

struct Completed {
    picture: Picture,
    summary: Summary,
}
struct Job {
    key: Key,
    cancel: Arc<AtomicBool>,
    rx: Receiver<Result<Option<Completed>, String>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct View {
    pub options_open: bool,
    window: Option<StereoWindow>,
    job: Option<Job>,
    shown_key: Option<Key>,
    pub shown: Option<Summary>,
    opened: Option<Instant>,
    warned: bool,
}

impl View {
    fn close(&mut self) {
        self.window = None;
        self.job = None;
        self.shown_key = None;
        self.shown = None;
        self.opened = None;
        self.warned = false;
    }

    pub fn inspect(&self) -> serde_json::Value {
        json!({"pending":self.job.is_some(),"shown":self.shown})
    }

    fn update(&mut self, app: &mut MaterialCraftApp, ctx: &egui::Context) -> Result<(), String> {
        if !app.session.stereo.enabled {
            self.close();
            return Ok(());
        }
        let size = render_size(app.session.stereo.resolution, app.view3d.native_rect, ctx.pixels_per_point())?;
        if self.window.is_none() {
            self.window = Some(StereoWindow::open(tr("Stereo View"), "ai.storyteller.materialcraft.StereoView").map_err(|e| e.to_string())?);
            self.opened = Some(Instant::now());
        }
        if self.window.as_ref().is_some_and(|w| !w.is_open()) {
            self.close();
            app.run("view.stereo", json!({"enabled":false}));
            return Ok(());
        }
        let Some(window) = &self.window else { return Ok(()) };
        let limits = window.depth_limits().map(|l| l.native_pixels(size[0]));
        let (sunk, popped) = limits.unwrap_or((0.0, 0.0));
        let budget = Budget { sunk: sunk as f32, popped: popped as f32 };
        if limits.is_none() && !self.warned && self.opened.is_some_and(|at| at.elapsed() >= Duration::from_secs(3)) {
            self.warned = true;
            app.toast(tr("Stereo depth is unavailable on this desktop; showing the flat model."), false);
        }
        if limits.is_some() {
            self.warned = false;
        }
        let relief = crate::view3d::update_relief(app, ctx);
        let key = (crate::view3d::content_key(app, size[0], size[1], app.view3d.show_env, relief), size, [budget.sunk.to_bits(), budget.popped.to_bits()]);
        if let Some(job) = self.job.take() {
            if job.key != key {
                job.cancel.store(true, Ordering::Relaxed);
            }
            match job.rx.try_recv() {
                Ok(Ok(Some(frame))) if job.key == key => {
                    if !window.show(frame.picture) {
                        return Err(tr("The stereo window closed before the pair was shown.").into());
                    }
                    self.shown_key = Some(key);
                    self.shown = Some(frame.summary);
                }
                Ok(Err(error)) if job.key == key => return Err(error),
                Err(TryRecvError::Empty) => {
                    // Cancel, then wait for acknowledgement. Never accumulate new workers
                    // while an obsolete row is still finishing on the shared Rayon pool.
                    self.job = Some(job);
                    ctx.request_repaint_after(Duration::from_millis(16));
                    return Ok(());
                }
                Err(TryRecvError::Disconnected) if job.key == key => return Err(tr("The stereo renderer stopped before completing its pair.").into()),
                _ => {}
            }
        }
        if self.shown_key != Some(key) {
            let Some((scene, light, maps, settings)) = crate::view3d::inputs(app, 2) else { return Err(tr("Could not prepare the material scene.").into()) };
            let cancel = Arc::new(AtomicBool::new(false));
            let (tx, rx) = std::sync::mpsc::channel();
            let stop = cancel.clone();
            let (repaint, viewport) = (ctx.clone(), ctx.viewport_id());
            std::thread::Builder::new()
                .name("materialcraft-stereo".into())
                .spawn(move || {
                    let result = materialcraft_render::stereo::render_pair_cancellable(&scene, &maps, &light, &settings, size[0], size[1], budget, &stop)
                        .and_then(|frame| {
                            frame
                                .map(|frame| {
                                    let summary = Summary {
                                        state: format!("{:016x}", key.0),
                                        size,
                                        budget: [budget.sunk, budget.popped],
                                        samples_per_eye: frame.samples_per_eye,
                                        primary_rays: frame.primary_rays,
                                        baseline: frame.camera.baseline,
                                        key_light_direction: frame.key_light.to_array(),
                                        key_light_strength: settings.key_light,
                                    };
                                    Picture::new(size[0] as usize * 2, size[1] as usize, frame.pixels.into(), true)
                                        .map(|picture| Completed { picture, summary })
                                        .map_err(|e| e.to_string())
                                })
                                .transpose()
                        });
                    let _ = tx.send(result);
                    repaint.request_repaint_of(viewport);
                })
                .map_err(|e| e.to_string())?;
            self.job = Some(Job { key, cancel, rx });
        }
        // Read live geometry/depth feedback even after the scene has settled.
        ctx.request_repaint_after(Duration::from_millis(16));
        Ok(())
    }

    fn settings(&mut self, app: &mut MaterialCraftApp, ctx: &egui::Context) {
        if !self.options_open {
            return;
        }
        let mut open = true;
        egui::Window::new(tr("Render resolution per eye")).open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label(tr("The 3D view size preserves its current sampling; the desktop scales once."));
            let Ok(mut size) = render_size(app.session.stereo.resolution, app.view3d.native_rect, ctx.pixels_per_point()) else {
                ui.label(tr("Open the 3D View first."));
                return;
            };
            let changed = ui
                .horizontal(|ui| {
                    ui.label(tr("Width"));
                    let width = ui.add(egui::DragValue::new(&mut size[0]).range(1..=4096)).changed();
                    ui.label(tr("Height"));
                    let height = ui.add(egui::DragValue::new(&mut size[1]).range(1..=4096)).changed();
                    width || height
                })
                .inner;
            if changed {
                app.run("view.stereo", json!({"resolution":size}));
            }
            if ui.button(tr("Use 3D view size")).clicked() {
                app.run("view.stereo", json!({"resolution":null}));
            }
        });
        self.options_open = open;
    }
}

pub fn tick(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    if app.stereo_view.is_none() && !app.session.stereo.enabled {
        return;
    }
    let mut view = app.stereo_view.take().unwrap_or_default();
    if let Err(error) = view.update(app, ctx) {
        view.close();
        app.run("view.stereo", json!({"enabled":false}));
        app.toast(trf("Could not show the stereo view: {error}", &[("error", &error)]), true);
    }
    if app.session.stereo.enabled || view.options_open {
        app.stereo_view = Some(view);
    }
}

pub fn settings(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    if let Some(mut view) = app.stereo_view.take() {
        view.settings(app, ctx);
        if app.session.stereo.enabled || view.options_open {
            app.stereo_view = Some(view);
        }
    }
}

/// The default uses the preview's existing physical grid and rounding rule.
pub fn render_size(requested: Option<[u32; 2]>, rect: Option<egui::Rect>, scale: f32) -> Result<[u32; 2], String> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(tr("The stereo view has invalid native dimensions.").into());
    }
    let size = match requested {
        Some(size) => size,
        None => {
            let rect = rect.filter(|r| r.is_finite() && r.is_positive()).ok_or_else(|| tr("Open the 3D View first.").to_owned())?;
            let (w, h) = crate::view3d::pixels(rect, scale);
            [w, h]
        }
    };
    if size.iter().any(|s| !(1..=4096).contains(s)) {
        return Err(tr("The stereo view has invalid native dimensions.").into());
    }
    Ok(size)
}

pub fn display_settings() -> Result<(), String> {
    let mut child =
        std::process::Command::new("kcmshell6").arg("kcm_kscreen").spawn().map_err(|e| trf("Could not open Display Settings: {error}", &[("error", &e)]))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
