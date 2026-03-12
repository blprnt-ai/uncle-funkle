use std::fs;

use tempfile::tempdir;
use uncle_funkle::{AssessmentImport, Config, State, SubjectiveFindingImport, UncleFunkle};

#[tokio::test]
async fn scan_merge_and_plan_smoke_test() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path();

    fs::write(
        root.join("main.rs"),
        r#"
        // TODO: split this file up
        fn huge() {
            println!("debug");
            if true {
                if true {
                    if true {
                        if true {
                            if true {
                                println!("nested");
                            }
                        }
                    }
                }
            }
        }
        "#,
    )
    .expect("write sample file");

    let engine = UncleFunkle::new(Config::default());
    let mut state = engine.load_state(root).await.expect("load state");
    let report = engine.scan(root).await.expect("scan report");
    let summary = engine.merge_scan(&mut state, report);

    assert!(summary.added > 0);
    assert!(state.stats.open_issues > 0);
    assert!(engine.next(&state).is_some());

    engine.save_state(root, &state).await.expect("save state");
    let reloaded = engine.load_state(root).await.expect("reload state");
    assert_eq!(reloaded.issues.len(), state.issues.len());
}

#[test]
fn subjective_assessment_changes_scores() {
    let engine = UncleFunkle::new(Config::default());
    let mut state = State::new();

    let summary = engine.import_subjective_assessment(
        &mut state,
        AssessmentImport {
            dimension: "architecture".to_string(),
            score: 42.0,
            summary: "Module boundaries are muddy".to_string(),
            findings: vec![SubjectiveFindingImport {
                path: Some("src/lib.rs".to_string()),
                summary: "Responsibilities are mixed".to_string(),
                description: "Parsing, orchestration, and persistence are coupled in one module"
                    .to_string(),
                ..SubjectiveFindingImport::default()
            }],
            ..AssessmentImport::default()
        },
    );

    assert!(summary.added >= 1);
    assert!(state.overall_score <= 100.0);
    assert!(state.verified_strict_score <= state.strict_score);
    assert!(state.subjective_assessments.contains_key("architecture"));
}
