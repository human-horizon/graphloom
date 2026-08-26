//! Live test against the real LLM endpoint from .env.
//! Run manually: `cargo test --test live -- --ignored --nocapture`
use graphloom_lib::pipeline;
use graphloom_lib::settings::Settings;
use std::env;
use std::path::Path;

const GO_SAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/go-sample");

#[tokio::test]
#[ignore = "requires a running LLM endpoint from .env"]
async fn live_project_map() {
    let settings = Settings::default();
    if let Ok(project) = env::var("GRAPHLOOM_LIVE_PROJECT") {
        let out = pipeline::analyze_project(Path::new(&project), &settings)
            .await
            .expect("large project analysis failed");
        eprintln!("large project report: {}", out.report_path.display());
        eprintln!("large project nodes: {} edges: {}", out.nodes, out.edges);
        assert!(out.nodes > 0);

        let file_out = pipeline::analyze_file(Path::new(&project), &settings, "main.go")
            .await
            .expect("large project file analysis failed");
        eprintln!("large project file report: {}", file_out.report_path.display());
        eprintln!("large project file nodes: {} edges: {}", file_out.nodes, file_out.edges);
        assert!(file_out.nodes > 0);
        return;
    }
    eprintln!("endpoint: {} model: {}", settings.endpoint.base_url, settings.endpoint.model);

    let models = graphloom_lib::llm::check_connection(&settings.endpoint)
        .await
        .expect("endpoint unreachable");
    eprintln!("models: {models:?}");

    let out = pipeline::analyze_project(Path::new(GO_SAMPLE), &settings)
        .await
        .expect("live project analysis failed");
    eprintln!("report: {}", out.report_path.display());
    eprintln!("nodes: {} edges: {}", out.nodes, out.edges);
    assert!(out.nodes > 0);

    let flow = pipeline::analyze_function(
        Path::new(GO_SAMPLE),
        &settings,
        "example.com/go-sample/internal/user.CreateUser",
    )
    .await
    .expect("live function flow failed");
    eprintln!("flow report: {}", flow.report_path.display());
    eprintln!("flow nodes: {} edges: {}", flow.nodes, flow.edges);
    let flow_html = std::fs::read_to_string(&flow.report_path).unwrap();
    assert!(flow.nodes > 0);
    assert!(!flow_html.contains("Запустить отправка"));
    assert!(flow_html.contains("Запустить фоновую отправку"));
}
