use regex::Regex;
use std::{env::args, error::Error, process::Command};

use rakune::{
    llm::{Ollama, LLM},
    repository::{Comment, Fragment, GitRepository, Transformation},
};

type Res<T> = Result<T, Box<dyn Error>>;

fn detect_language() -> String {
    "Rust".to_string()
}

struct RustBuilder<const N: usize> {
    /// Command arguments to run in order to build the project
    command: [&'static str; N],
}
impl<const N: usize> RustBuilder<N> {
    /// Builds the repository and returns an optional string of the build output if the build was
    /// not successful, else do not return anything
    fn build(&self, _: &GitRepository) -> Result<(), Vec<Comment>> {
        let output = Command::new(self.command[0])
            .args(&self.command[1..])
            .output()
            .expect(&format!("failed to call build command {:?}", self.command));

        if let Some(code) = output.status.code() {
            if code == 0 {
                return Ok(());
            }
        }

        let output = std::str::from_utf8(&output.stderr).expect("failed to read stderr");

        let file_regex =
            Regex::new("error: (.*?)\n --> (.*?):(\\d+):(\\d+)").expect("Regex failed to compile.");

        let errors = file_regex
            .captures_iter(output)
            .map(|c| c.extract())
            .map(|(_, [error, file, line_no, _])| Comment {
                message: Prompter::template_debug(error),
                fragments: vec![Fragment {
                    filepath: file.to_string(),
                    line_range: (
                        line_no.parse::<usize>().unwrap() - 1,
                        line_no.parse::<usize>().unwrap(),
                    ),
                }],
            })
            .collect();

        #[cfg(debug_assertions)]
        eprintln!("################################# {:?}", errors);

        Err(errors)
    }
}

/// performs the actions to edit the code in the repository
pub struct Coder<M: LLM> {
    pub transformation_count: usize,
    pub repository: GitRepository,
    pub llm: M,
}

impl<T: LLM> Coder<T> {
    fn prompt(&self, prompt: &str) -> Res<String> {
        self.llm.prompt(&prompt)
    }

    // prompt -> embedding -> context(s) (code blocks fetched by the embedding)
    //
    // when you use a particular context "block", if it fails or succeeds the
    // build/validation, then it will decrease or increase its score related to the prompt embedding

    fn generate_transformations(&mut self, comment: &Comment) -> Res<Vec<Transformation>> {
        let mut prompt = Prompter::template_code(&comment.message);

        if let Some(fragment) = comment.fragments.get(0) {
            prompt += "\n---\n";

            let _temporal_context = self.repository.temporal_context(&fragment)?;
            let spatial_context = self.repository.spatial_context(&fragment)?;

            for context in _temporal_context {
                prompt += "\n";
                prompt += &context;
            }

            for context in spatial_context {
                prompt += "\n";
                prompt += &context;
            }
        }

        let answer = self.prompt(&prompt)?;

        // TODO: jump from answer to transformations
        // use the answer to construct a sequence of transformations

        let transformations = vec![Transformation::try_from(answer.as_str())?];

        transformations
            .iter()
            .try_for_each(|t| self.repository.transform(t))?;

        Ok(transformations)
    }

    fn generate_commit(&self, repo: &GitRepository) -> Res<String> {
        // summarize the diff when creating a commit message
        let diff = repo.diff(None)?;
        let prompt = &format!(
            "summarize the following diff as a commit message in less than 20 words:\n\n{}",
            diff
        );
        self.prompt(prompt)
    }
}

struct Prompter;
impl Prompter {
    fn template_code(p: &str) -> String {
        format!(
            "You are a {} programmer. {}

Please use the following template to describe where to update the code:

```
UpdateFragment:
    filepath: the path to the file being changes (string)
    start_line: the starting line to update (int)
    end_line: the ending line to update (int)
    content: the code the replace within the lines (string)
```

Do NOT provide any extra content beyond this template.",
            detect_language(),
            p
        )
    }

    fn template_debug(p: &str) -> String {
        format!("fix this build error:\n\n{}", p)
    }
}

// emulated a single comment on a current state of the repository
fn main() -> Res<()> {
    let args = args().collect::<Vec<_>>();

    let mut comments = vec![Comment {
        message: args[1].clone(),
        fragments: vec![Fragment {
            filepath: "src/test.rs".to_string(),
            line_range: (0, 1),
        }],
    }];

    let repo = GitRepository::default();
    let builder = RustBuilder {
        command: ["cargo", "build"],
    };
    let ollama = Ollama {
        model: "codellama:7b-instruct",
        endpoint: "http://localhost:11434/api/generate",
    };

    let mut coder = Coder {
        transformation_count: 2,
        repository: repo,
        llm: ollama,
    };

    while let Some(comment) = comments.pop() {
        coder.generate_transformations(&comment)?;

        // self-correct until the program compiles
        while let Err(errors) = builder.build(&coder.repository) {
            if let Some(error) = errors.first() {
                coder.generate_transformations(&error)?;
            }
        }
    }

    let _commit_message = coder.generate_commit(&coder.repository)?;

    Ok(())
}
