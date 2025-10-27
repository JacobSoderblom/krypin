use hubd::{config::Config, run};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::from_env()?;
    run(cfg).await
}
