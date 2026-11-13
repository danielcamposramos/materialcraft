//! Identical ordinary UI and scene inputs in the candidate and pristine stock.
use egui_kittest::Harness;
use materialcraft_engine::{execute, sample_session};
use materialcraft_ui_egui::{MaterialCraftApp, UiState};
use serde_json::json;

#[test]
#[ignore = "requires private software GPU and SCENE4_FIXTURE/SCENE4_STOCK_SNAPSHOT"]
fn ordinary_ui_with_controlled_timing() {
    let fixture = std::path::PathBuf::from(std::env::var_os("SCENE4_FIXTURE").expect("SCENE4_FIXTURE"));
    let output = std::path::PathBuf::from(std::env::var_os("SCENE4_STOCK_SNAPSHOT").expect("SCENE4_STOCK_SNAPSHOT"));
    std::fs::create_dir_all(&output).unwrap();
    let mut session = sample_session();
    execute(&mut session, "file.new", &json!({"name":"Stereo proof","size":16})).unwrap();
    execute(&mut session, "view.3d.loadMesh", &json!({"path":fixture.join("two-depth.obj")})).unwrap();
    let bitmap = execute(&mut session, "file.importBitmap", &json!({"path":fixture.join("two-colors.png")})).unwrap();
    let node = execute(&mut session, "graph.resource.add", &json!({"resource":bitmap["name"]})).unwrap()["id"].clone();
    execute(&mut session, "view.3d.assign", &json!({"node":node,"channel":"baseColor","reset":true})).unwrap();
    execute(&mut session, "view.3d.assign", &json!({"node":node,"channel":"emissive"})).unwrap();
    execute(
        &mut session,
        "view.3d",
        &json!({"yaw":0,"pitch":0,"distance":5.0/1.45_f64.sqrt(),"target":[0,0,0],"heightScale":0,"environment":"studio","axis":false,"grid":false}),
    )
    .unwrap();
    session.cook();
    let app = MaterialCraftApp::headless(session, UiState::default());
    let mut harness = Harness::builder().with_size(egui::vec2(1000.0, 680.0)).with_pixels_per_point(1.0).wgpu().build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            // Public test inputs fix only diagnostic clocks; every output pixel is checked.
            app.session.cooked.ms = 0.0;
            app.view3d.render_ms = 0.0;
            app.ui_ms = 0.0;
            app.draw(ui);
        },
        app,
    );
    for _ in 0..80 {
        harness.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(!harness.state().cooker.busy());
    let first = harness.render().unwrap();
    first.save(output.join("whole-ui.png")).unwrap();
    for _ in 0..4 {
        harness.run_steps(1);
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let second = harness.render().unwrap();
    second.save(output.join("whole-ui-stable.png")).unwrap();
    assert_eq!(first.dimensions(), (1000, 680));
    assert_eq!(first.as_raw(), second.as_raw(), "ordinary UI must have settled");
    let (w, h, pixels) = harness.state().view3d.shown.as_ref().expect("ordinary CPU viewport");
    image::save_buffer(output.join("ordinary-viewport.png"), pixels, *w, *h, image::ColorType::Rgba8).unwrap();
}
