use materialcraft_engine::{execute, sample_session};
use serde_json::json;

#[test]
fn stereo_options_are_atomic_view_state_and_leave_the_document_unchanged() {
    let mut s = sample_session();
    let document = serde_json::to_value(&s.pkg).unwrap();
    let before = materialcraft_engine::inspect::session(&s, None);
    let settings = execute(&mut s, "view.stereo", &json!({"enabled":true,"resolution":[640,360]})).unwrap();
    assert_eq!(settings, json!({"enabled":true,"resolution":[640,360]}));
    assert_eq!(materialcraft_engine::inspect::session(&s, None)["stereo"], settings);
    for bad in [
        json!({"resolution":[0,360]}),
        json!({"resolution":[4097,360]}),
        json!({"resolution":[640]}),
        json!({"enabled":"yes"}),
        json!({"enabled":false,"resolution":[1,-1]}),
    ] {
        assert!(execute(&mut s, "view.stereo", &bad).is_err());
        assert_eq!(execute(&mut s, "view.stereo", &json!({})).unwrap(), settings);
    }
    assert_eq!(execute(&mut s, "view.stereo", &json!({"resolution":null})).unwrap(), json!({"enabled":true,"resolution":null}));
    assert_eq!(serde_json::to_value(&s.pkg).unwrap(), document);
    let after = materialcraft_engine::inspect::session(&s, None);
    assert_eq!(before["history"], after["history"]);
    assert_eq!(before["view3d"], after["view3d"]);
    assert_eq!(before["package"], after["package"]);
}
