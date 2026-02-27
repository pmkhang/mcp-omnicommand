use serde_json::{Value, json};

pub mod fetch_api;
pub mod find_file;
pub mod list_directory;
pub mod process_kill;
pub mod process_list;
pub mod run_command;
pub mod tail_file;
pub mod wait_for;

pub fn get_tools() -> Value {
    json!([
        run_command::info(),
        fetch_api::info(),
        process_list::info(),
        process_kill::info(),
        list_directory::info(),
        find_file::info(),
        tail_file::info(),
        wait_for::info(),
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
        "list_directory" => list_directory::run(arguments),
        "find_file" => find_file::run(arguments),
        "tail_file" => tail_file::run(arguments),
        "wait_for" => wait_for::run(arguments).await,
        _ => Err(format!("Unknown tool: {name}")),
    }
}
