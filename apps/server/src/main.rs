use clap::Parser;
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> anyhow::Result<()> {
    let cli = pley::config::Cli::parse();
    let config = pley::config::Config::load(&cli)?;
    match cli.command {
        None | Some(pley::config::Command::Serve) => {
            pley::logging::init(&cli.log, Some(&config.log_file()));
            pley::serve(config, !cli.no_tray)
        }
        Some(command) => {
            pley::logging::init(&cli.log, None);
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
            runtime.block_on(async move {
                match command {
                    pley::config::Command::ResetPassword { username, password, admin } => {
                        pley::reset_password(config, &username, &password, admin).await
                    }
                    pley::config::Command::Health => pley::health(&config).await,
                    pley::config::Command::Serve => unreachable!(),
                }
            })
        }
    }
}
