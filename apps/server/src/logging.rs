use tracing_subscriber::{EnvFilter, fmt};

/// Initialise tracing. `filter` uses `EnvFilter` syntax; a bare level such as
/// `info` keeps noisy dependencies at `warn`.
pub fn init(filter: &str) {
    let filter = if filter.contains('=') || filter.contains(',') {
        filter.to_owned()
    } else {
        format!("{filter},sqlx=warn,hyper=warn,tower_http=warn,reqwest=warn")
    };
    let env_filter = EnvFilter::try_new(&filter).unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(env_filter).with_target(false).compact().try_init();
}
