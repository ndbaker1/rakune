use std::error::Error;

use reqwest;
use serde::{Deserialize, Serialize};

pub trait LLM {
    fn prompt(&self, prompt: &str) -> Result<String, Box<dyn Error>>;
}

#[derive(Deserialize)]
struct OllamaResponse {
    model: String,
    created_at: String,
    response: String,
    done: bool,
}

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: String,
    prompt: String,
    stream: bool,
    context: &'a [usize],
}

pub struct Ollama<'a> {
    pub endpoint: &'a str,
    pub model: &'a str,
}
impl LLM for Ollama<'_> {
    fn prompt(&self, prompt: &str) -> Result<String, Box<dyn Error>> {
        let client = reqwest::blocking::Client::new();

        let request_body = serde_json::to_string(&OllamaRequest {
            prompt: prompt.to_string(),
            model: self.model.to_string(),
            stream: false,
            context: &[],
        })?;

        let response = client
            .post(self.endpoint)
            .body(request_body)
            .send()?
            .text()?;

        let response = serde_json::from_str::<OllamaResponse>(&response)?;

        #[cfg(debug_assertions)]
        eprintln!("*****************************\n{:#}\n", response.response);

        Ok(response.response.to_string())
    }
}
