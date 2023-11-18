use std::error::Error;

use reqwest;
use serde::{Deserialize, Serialize};

pub trait LLM {
    fn prompt(&self, prompt: &str) -> Result<String, Box<dyn Error>>;
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
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

        let ollama_request = &OllamaRequest {
            prompt: prompt.to_string(),
            model: self.model.to_string(),
            stream: false,
            context: &[],
        };

        #[cfg(debug_assertions)]
        eprintln!(
            "*************** Payload **************\n{:#}\n",
            ollama_request.prompt
        );

        let response = client
            .post(self.endpoint)
            .body(serde_json::to_string(ollama_request)?)
            .send()?
            .text()?;

        let response = serde_json::from_str::<OllamaResponse>(&response)?;

        #[cfg(debug_assertions)]
        eprintln!(
            "*************** Response **************\n{:#}\n",
            response.response
        );

        Ok(response.response.to_string())
    }
}

