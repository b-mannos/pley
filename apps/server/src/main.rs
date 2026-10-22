use clap::Parser;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> anyhow::Result<()> {
    let cli = pley::config::Cli::parse();
    pley::logging::init(&cli.log);

    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("pley-worker").build()?;

    runtime.block_on(async move {
        let config = pley::config::Config::load(&cli)?;
        match cli.command {
            None | Some(pley::config::Command::Serve) => pley::run(config).await,
            Some(pley::config::Command::ResetPassword { username, password, admin }) => {
                pley::reset_password(config, &username, &password, admin).await
            }
            Some(pley::config::Command::Health) => pley::health(&config).await,
        }
    })
}
