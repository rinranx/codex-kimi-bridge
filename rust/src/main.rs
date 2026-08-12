#[tokio::main]
async fn main() {
    let code = codex_kimi_bridge::cli::run(std::env::args().skip(1).collect()).await;
    std::process::exit(code);
}
