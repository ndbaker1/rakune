use regex::Regex;
use std::{env::args, error::Error, process::Command};

mod test;

use rakune::{
    llm::{Ollama, LLM},
    repository::{Comment, Fragment, GitRepository, Transformation},
};

type Res<T> = Result<T, Box<dyn Error>>;

fn detect_language() -> String {
    "Rust".to_string()
}

struct RustBuilder<'a> {
    /// Command arguments to run in order to build the project
    build_args: &'a [&'a str],
    lint_args: &'a [&'a str],
}
impl RustBuilder<'_> {
    /// Builds the repository and returns an optional string of the build output if the build was
    /// not successful, else do not return anything
    fn build(&self, _: &GitRepository) -> Result<(), Vec<Comment>> {
        Command::new(self.lint_args[0])
            .args(&self.lint_args[1..])
            .output()
            .expect(&format!(
                "failed to execute lint command {:?}",
                self.lint_args
            ));

        let output = Command::new(self.build_args[0])
            .args(&self.build_args[1..])
            .output()
            .expect(&format!(
                "failed to call build command {:?}",
                self.build_args
            ));

        if let Some(code) = output.status.code() {
            if code == 0 {
                return Ok(());
            }
        }

        let output = std::str::from_utf8(&output.stderr).expect("failed to read stderr");

        let file_regex = Regex::new("error: ([\\s\\S]*?)\n --> (.*?):(\\d+):(\\d+)")
            .expect("Regex failed to compile.");

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
            prompt += "\n### Here is the current context:\n";

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

        // TODO: jump from answer to transformations
        // use the answer to construct a sequence of transformations

        let mut transformations = Vec::new();
        while transformations.is_empty() {
            let answer = self.prompt(&prompt)?;
            transformations = Transformation::parse_from(answer.as_str())?;
        }

        assert!(!transformations.is_empty());

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
            r#"You are a {} programmer. {}

Please use the following template to describe where to update the code:

```
UpdateFragment:
    filepath: the path to the file being changes (string)
    start_line: the starting line to update (int)
    end_line: the ending line to update (int)
    content: the code the replace within the lines (string)
```

Do NOT provide any extra content beyond this template.

## Here are a couple of examples:

Update the function foo to print "hello!"

>>>>
0 fn foo() {{
1     println!("chili dogs")
2 }}
<<<<

```
UpdateFragment:
    filepath: src/hello.rs
    start_line: 1
    end_line: 1
    content: println!("hello!")
```

---

Remove the uneeded code in add_5().

>>>>
0 fn add_5(x: u8) -> u8 {{
1   let ans = x + 5;
2   return ans;
3 }}
<<<<

```
UpdateFragment:
    filepath: src/addition.rs
    start_line: 1
    end_line: 2
    content: return x + 5;
```
"#,
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
            line_range: (0, 6),
        }],
    }];

    let repo = GitRepository::default();
    let builder = RustBuilder {
        build_args: &["cargo", "build"],
        lint_args: &["cargo", "fmt"],
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
