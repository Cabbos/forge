use std::io::Read;

#[tokio::main]
async fn main() {
    let mut input = String::new();
    if let Err(error) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("failed to read Forge eval stdin: {error}");
        std::process::exit(1);
    }

    match forge::eval_headless::run_stdin_json(&input).await {
        Ok(payload) => {
            println!(
                "{}",
                serde_json::to_string(&payload)
                    .unwrap_or_else(|_| "{\"failure_category\":\"serialization_error\"}".into())
            );
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
