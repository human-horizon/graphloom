//! E2E: full pipeline against the mock OpenAI endpoint and fixture projects.
use graphloom_lib::pipeline;
use graphloom_lib::settings::Settings;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures");

struct MockServer(Child);

impl MockServer {
    // Child is reaped in Drop (kill + wait); clippy cannot see through Drop.
    #[allow(clippy::zombie_processes)]
    fn start() -> Self {
        let server = Command::new("python3")
            .arg(format!("{FIXTURES}/mock-llm/server.py"))
            .spawn()
            .expect("failed to spawn mock LLM server");
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if TcpStream::connect("127.0.0.1:8399").is_ok() {
                return Self(server);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("mock LLM server did not start");
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn mock_settings() -> Settings {
    let mut settings = Settings::default();
    settings.endpoint.base_url = "http://127.0.0.1:8399/v1".to_string();
    settings.endpoint.model = "mock-model".to_string();
    settings
}

#[tokio::test]
async fn full_pipeline_against_mock_llm() {
    let _server = MockServer::start();
    let settings = mock_settings();
    let go_sample = PathBuf::from(format!("{FIXTURES}/go-sample"));
    let _ = std::fs::remove_dir_all(go_sample.join(".graphloom"));

    // Project map
    let out = pipeline::analyze_project(&go_sample, &settings)
        .await
        .expect("project analysis failed");
    assert!(out.report_path.exists());
    assert_eq!(out.nodes, 5);
    assert_eq!(out.edges, 0);
    let html = std::fs::read_to_string(&out.report_path).unwrap();
    assert!(html.contains("Работа с пользователями"));
    assert!(html.contains("Создать пользователя"));

    // Determinism: same input -> identical HTML bytes
    let second = pipeline::analyze_project(&go_sample, &settings)
        .await
        .expect("second project analysis failed");
    let html2 = std::fs::read_to_string(&second.report_path).unwrap();
    assert_eq!(html, html2);

    // File diagram
    let file_out = pipeline::analyze_file(&go_sample, &settings, "internal/user/user.go")
        .await
        .expect("file analysis failed");
    assert!(file_out.report_path.exists());
    assert!(file_out.nodes >= 8);

    // Function flow
    let flow = pipeline::analyze_function(
        &go_sample,
        &settings,
        "example.com/go-sample/internal/user.CreateUser",
    )
    .await
    .expect("function flow failed");
    assert!(flow.report_path.exists());
    assert!(flow.nodes >= 3);
    let flow_html = std::fs::read_to_string(&flow.report_path).unwrap();
    assert!(flow_html.contains("CreateUser"));
    assert!(flow_html.contains("internal/user/user.go"));

    // Symbol listing for the UI picker
    let symbols = pipeline::get_symbols(Path::new(&format!("{FIXTURES}/go-sample")))
        .await
        .expect("get_symbols failed");
    assert!(symbols
        .iter()
        .any(|s| s.id == "example.com/go-sample/internal/user.CreateUser"));

    // TS project map
    let ts_sample = PathBuf::from(format!("{FIXTURES}/ts-sample"));
    let ts_settings = mock_settings();
    let ts_out = pipeline::analyze_project(&ts_sample, &ts_settings)
        .await
        .expect("ts project analysis failed");
    let ts_html = std::fs::read_to_string(&ts_out.report_path).unwrap();
    assert!(ts_html.contains("Сервис пользователей"));    // Incremental file generation: cache hit, then content change invalidation.
    let initial_plan = pipeline::get_update_plan(&go_sample, &settings)
        .await
        .expect("update plan failed");
    let tracked = initial_plan
        .files
        .iter()
        .find(|file| file.path == "internal/user/user.go")
        .expect("tracked file missing")
        .clone();
    pipeline::update_file(&go_sample, &settings, &tracked.path, &tracked.hash)
        .await
        .expect("incremental update failed");
    let cached_plan = pipeline::get_update_plan(&go_sample, &settings)
        .await
        .expect("cached update plan failed");
    assert_eq!(cached_plan.files.iter().find(|file| file.path == tracked.path).unwrap().status, "ready");

    let source_path = go_sample.join(&tracked.path);
    let original = std::fs::read_to_string(&source_path).unwrap();
    std::fs::write(&source_path, format!("{original}\n// changed for incremental test\n")).unwrap();
    let changed_plan = pipeline::get_update_plan(&go_sample, &settings)
        .await
        .expect("changed update plan failed");
    assert_eq!(changed_plan.files.iter().find(|file| file.path == tracked.path).unwrap().status, "pending");
    std::fs::write(source_path, original).unwrap();

    // Render pipeline: stored DSL should re-render to HTML without touching the LLM.
    let rerendered = pipeline::render_report_from_dsl(&go_sample, &settings, &tracked.path)
        .expect("rerender from DSL failed");
    let new_html = std::fs::read_to_string(&rerendered).unwrap();
    assert!(new_html.contains("#layers { display: flex"));
    assert!(new_html.contains(".elabel-bg"));
    assert!(new_html.contains("CreateUser"));

    let _ = std::fs::remove_dir_all(go_sample.join(".graphloom"));
}
