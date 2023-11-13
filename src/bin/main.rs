use std::error::Error;

use rakune::{
    llm::Ollama,
    repository::{Coder, GitRepository},
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut repo = GitRepository::default();

    let coder = Coder {
        transformation_count: 2,
        llm: Ollama {
            model: "codellama:7b-instruct",
            endpoint: "http://localhost:11434/api/generate",
        },
    };

    let instructions = String::from("write a rust function to call openapi");

    let transformations = coder.generate_transformations(&instructions)?;
    transformations.iter().try_for_each(|t| repo.transform(t))?;

    // test to see if the program compiles
    while let Ok(Some(build_output)) = repo.build() {
        let instructions =
            coder.prompt(&format!("fix the error in this build:\n\n{}", build_output))?;

        let transformations = coder.generate_transformations(&instructions)?;
        transformations.iter().try_for_each(|t| repo.transform(t))?;
    }

    // summarize the diff when creating a commit message
    let diff = repo.diff(None)?;
    let commit_message = coder.prompt(&format!(
        "summarize the following diff as a commit message in less than 20 words:\n\n{}",
        diff
    ))?;

    Ok(())
}
