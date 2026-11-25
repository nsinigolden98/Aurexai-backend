use std::sync::Arc;

use migration::MigratorTrait;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use aurexai_backend::{app::AppState, config::Config, db, services};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aurexai_backend=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env().map_err(|e| {
        eprintln!("Configuration error: {e}");
        std::io::Error::new(std::io::ErrorKind::InvalidInput, e)
    })?;

    let db = db::connect(&config.database_url).await?;
    migration::Migrator::up(&db, None).await?;

    let email_service = services::email::EmailService::new(&config);
    let notification_manager = services::notification::NotificationManager::new();
    let state = AppState {
        db,
        config: Arc::new(config.clone()),
        email_service,
        notification_manager,
    };

    let app = aurexai_backend::app::create_router(state);
    let addr = format!("{}:{}", config.app_host, config.app_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("listening on http://{addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
