mod cli;
mod process;
mod server;
mod tools;

pub const SERVER_VERSION: &str = "1.1.0";

use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Chạy ở chế độ CLI trực tiếp nếu có đối số và đối số đầu tiên không phải là "@mcp"
    if args.len() >= 2 && args[1] != "@mcp" {
        cli::run_standalone_cli(&args).await;
        return;
    }

    server::run_mcp_server().await;
}
