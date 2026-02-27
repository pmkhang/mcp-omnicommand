use serde_json::{Value, json};

pub mod process_cleanup;
pub mod process_kill;
pub mod process_list;
pub mod run_command;
pub mod system_info;

pub fn get_tools() -> Value {
    json!([
        run_command::info(),
        system_info::info(),
        process_list::info(),
        process_cleanup::info(),
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
        "system_info" => system_info::run(arguments).await,
        "process_list" => process_list::run(arguments).await,
        "process_cleanup" => process_cleanup::run(arguments).await,
        "process_kill" => process_kill::run(arguments).await,
        _ => Err(format!("Unknown tool: {}", name)),
    }
}
