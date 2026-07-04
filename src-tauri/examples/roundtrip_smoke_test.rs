//! Manual smoke test for save_project/load_project round-tripping and
//! schema-version rejection. Run with:
//!   PROJECT_JSON_PATH=/path/to/project.json cargo run --example roundtrip_smoke_test
fn main() {
    let path = std::env::var("PROJECT_JSON_PATH").expect("set PROJECT_JSON_PATH");
    let project = video_editor_app_lib::commands::load_project(path.clone())
        .expect("load_project should parse a freshly-imported project");

    let tmp_out = format!("{path}.roundtrip.json");
    video_editor_app_lib::commands::save_project(project.clone(), tmp_out.clone())
        .expect("save_project should succeed");
    let reloaded = video_editor_app_lib::commands::load_project(tmp_out)
        .expect("load_project should parse what save_project just wrote");

    assert_eq!(project.source_files.len(), reloaded.source_files.len());
    assert_eq!(project.clips.len(), reloaded.clips.len());
    assert_eq!(project.schema_version, reloaded.schema_version);
    println!("round-trip OK: {} source files, {} clips, schemaVersion={}", reloaded.source_files.len(), reloaded.clips.len(), reloaded.schema_version);

    // Schema-version rejection check.
    let bad = path.replace(".json", ".badversion.json");
    let mut bad_project = reloaded;
    bad_project.schema_version = 999;
    std::fs::write(&bad, serde_json::to_string(&bad_project).unwrap()).unwrap();
    match video_editor_app_lib::commands::load_project(bad) {
        Err(e) => println!("correctly rejected future schema version: {e}"),
        Ok(_) => panic!("load_project should have rejected an unsupported schema version"),
    }
}
