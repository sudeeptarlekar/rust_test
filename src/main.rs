use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Response {
    pub model: String,
    pub created_at: String,
    pub response: String,
    pub done: bool,
    pub done_reason: String,
    pub total_duration: usize,
    pub load_duration: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let file_path: &str = "/Users/sudeep.tarlekar/Downloads/logs/plain_dlt.txt";
    let ollama_url = "http://localhost:11434/api/generate";
    let client = Client::new();
    let lines = std::fs::read_to_string(file_path)?
        .lines()
        .map(String::from)
        .collect::<Vec<String>>();

    let file_data = lines.get(0..100).map(|logs| logs.join("\n")).unwrap();

    let response: Response = client
        .post(ollama_url)
        .json(&json!({
            "model": "llama3",
            "prompt": format!("Give me regex for filtering out error lines from this log: {}", file_data),
            "stream": false
        }))
        .send()
        .await?
        .json()
        .await?;

    println!("Response : {:?}", response);
    Ok(())
}
