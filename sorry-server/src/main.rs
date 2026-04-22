use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::http::HeaderValue;
use clap::Parser;

use sorry_server::{AppStateInner, build_app, lobby::Lobby};

#[derive(Parser)]
#[command(name = "sorry-server")]
struct Args {
    /// Port to listen on.
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Origin allowed by CORS. Pass "*" for any, or a full origin like
    /// "http://localhost:5173" for the SvelteKit dev server. Omit to disable
    /// CORS entirely (same-origin only).
    #[arg(long, default_value = "http://localhost:5173")]
    cors_origin: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sorry_server=info,tower_http=info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    let app_state = Arc::new(AppStateInner {
        lobby: Lobby::new(100),
    });

    let cleanup_state = app_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_state.lobby.cleanup_stale_rooms(
                Duration::from_secs(300),
                Duration::from_secs(600),
            );
        }
    });

    let cors_origin = match args.cors_origin.as_str() {
        "" | "none" => None,
        origin => Some(
            origin
                .parse::<HeaderValue>()
                .expect("invalid --cors-origin value"),
        ),
    };

    let app = build_app(app_state, cors_origin);

    let addr = format!("0.0.0.0:{}", args.port);
    tracing::info!("Starting sorry-server on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");
}
