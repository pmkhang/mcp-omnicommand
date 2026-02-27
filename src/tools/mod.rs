use serde_json::{Value, json};

pub mod fetch_api;
pub mod process_kill;
pub mod process_list;
pub mod run_command;

pub fn get_tools() -> Value {
    json!([
        run_command::info(),
        fetch_api::info(),
        process_list::info(),
        process_kill::info(),
    ])
}

pub async fn handle_tool_call(
    name: &str,
    arguments: &Value,
    default_cwd: Option<&str>,
) -> Result<Value, String> {
    match name {
        "run_command" => run_command::run(arguments, default_cwd).await,
        "fetch_api" => fetch_api::run(arguments).await,
        "process_list" => process_list::run(arguments),
        "process_kill" => process_kill::run(arguments),
        _ => Err(format!("Unknown tool: {name}")),
    }
}
