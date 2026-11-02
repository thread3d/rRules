//! Main entry point for rRules REST API server

use actix_web::HttpServer;

// The modules live in the library (`lib.rs`); the binary is only process wiring.
use api::v1;

// The background sweep task that expires idle sessions.
use api::v1::sessions::run_session_sweep;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // `RRULES_BIND_ADDRESS` and `PORT` decide where the server listens. The
    // default is loopback, which is right for a laptop and wrong for a container:
    // an image sets `RRULES_BIND_ADDRESS=0.0.0.0` so the port is reachable.
    let bind_address = {
        let host = std::env::var("RRULES_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        format!("{host}:{port}")
    };

    println!("rRules REST API Server v9.0.84");
    println!("Starting on http://{bind_address}");

    // One assembly, shared with the tests: `from_paths` builds every store, the engine registry,
    // the shared rule set and the DMN model, so a test cannot run against a differently wired server.
    let state = api::app::from_environment().await?;
    let ruleset_inner = state.ruleset.get_ref().clone();
    let sweep_manager = state.session_manager.clone();
    let rules_dir = std::env::var("RULE_STORAGE_PATH").unwrap_or_else(|_| "./rules".to_string());

    // Start the rules-directory watcher unless it is turned off.
    let watch_rules = std::env::var("WATCH_RULES_DIR")
        .map(|value| value == "true" || value == "1")
        .unwrap_or(true);
    if watch_rules {
        println!("Watching the rules directory: {rules_dir}");
        tokio::spawn(v1::execution::start_file_watcher(
            ruleset_inner.clone(),
            rules_dir,
        ));
    }

    println!(
        "Starting session sweep task (interval: {:?})",
        sweep_manager.sweep_interval
    );
    tokio::spawn(run_session_sweep(sweep_manager));

    let server = HttpServer::new(move || api::app::build_app(&state)).bind(&bind_address)?;

    println!("Server running at http://{bind_address}");
    server.run().await
}
