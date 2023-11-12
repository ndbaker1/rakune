use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::process::Command;

use crate::llm::LLM;
use crate::DataSource;
use crate::Diff;
use crate::Result;
use crate::Step;

#[derive(Clone)]
pub struct GitRepository {
    /// If the revisions is `None`, then we are at HEAD, else the sha hash of the revision will be
    /// stored in the option.
    revision: Option<String>,
}

impl Default for GitRepository {
    fn default() -> Self {
        Self { revision: None }
    }
}

impl GitRepository {
    /// Edit the state of a respository using a given agent capability
    pub fn transform(&mut self, transformation: &Transformation) -> Result<()> {
        Ok(match transformation {
            Transformation::UpdateFragment {
                filepath,
                line_range,
                content,
            } => {
                let mut file = File::open(filepath)?;

                let mut contents = String::new();
                file.read_to_string(&mut contents)?;

                let mut lines: Vec<_> = contents.lines().collect();
                lines.splice(line_range.0..line_range.1, content.into_iter().cloned());
                file.write_all(&lines.join("\n").as_bytes())?;
            }
            _ => unreachable!(),
        })
    }

    pub fn diff(&self, target: Option<&Self>) -> Result<Diff> {
        let mut command = Command::new("git");
        let command = match target {
            Some(other) => command.args(&[
                "diff",
                &self.revision.clone().unwrap(),
                &other.revision.clone().unwrap(),
            ]),
            None => command.args(&["diff"]),
        };

        let output = command.output()?;
        let output = std::str::from_utf8(&output.stdout)?.to_string();
        Ok(output)
    }

    pub fn commit(&mut self, commit_message: &str) -> Result<String> {
        Command::new("git").args(&["add", "."]).output()?;
        Command::new("git")
            .args(&["commit", "-m", commit_message])
            .output()?;
        let commit_revision = Command::new("git").args(&["rev-parse", "HEAD"]).output()?;
        let commit_revision = std::str::from_utf8(&commit_revision.stdout)?.to_string();
        Ok(commit_revision)
    }

    /// Builds the repository and returns an optional string of the build output if the build was
    /// not successful, else do not return anything
    pub fn build(&mut self) -> Result<Option<String>> {
        let output = Command::new("cargo").args(&["test"]).output()?;

        if let Some(code) = output.status.code() {
            if code == 0 {
                return Ok(None);
            }
        }

        let output = std::str::from_utf8(&output.stdout)?.to_string();

        Ok(Some(output))
    }

    /// Searches through git or conversation history for context on a particular code fragment
    pub fn temporal_context() {
        unimplemented!()
    }

    /// Searches through symbolic, lexical, or etc information on a particular code fragment
    pub fn spatial_context() {
        unimplemented!()
    }
}

type LineRange = (usize, usize);

struct RepositoryPrompt {
    comment: String,
    fragments: Vec<LineRange>,
}

impl DataSource<Query, QueryResponse> for GitRepository {
    fn query(&self, query: &Query) -> Result<QueryResponse> {
        Ok(match query {
            Query::None => QueryResponse::None,
        })
    }
}

pub enum Transformation<'s> {
    RenameSymbol {
        old: String,
        new: String,
    },
    CreateFile {
        path: String,
    },
    DeleteFile {
        path: String,
    },
    MoveFile {
        old: String,
        new: String,
    },
    UpdateFragment {
        filepath: String,
        line_range: LineRange,
        content: &'s [&'s str],
    },
}

enum Feedback {
    Fragment,
    Guidance,
    Holistic,
}

enum Query {
    None,
}

enum QueryResponse {
    None,
}

/// performs the actions to edit the code in the repository
pub struct Coder<M: LLM> {
    // memory_context: (),
    pub transformation_count: usize,
    pub llm: M,
}
impl<T: LLM> Coder<T> {
    pub fn prompt(&self, prompt: &str) -> Result<String> {
        self.llm.prompt(&prompt)
    }

    pub fn generate_transformations(&self, step: &Step) -> Result<Vec<Transformation>> {
        let prompt = format!(
            r#"for the following request, provide an answer in the format:

        UpdateFragment:
            filepath: string
            start_line: integer
            end_line: integer
            content: string

        Here is the request to generate answers for:

        {}"#,
            step
        );

        let _answer = self.prompt(&prompt)?;

        Ok(vec![])
    }
}

type Validation = ();

/// compiles and runs the code to give feedback to the coding agent
struct Tester;
impl Tester {
    fn generate_validations(&self, _diff: &Diff) -> Vec<Validation> {
        unimplemented!()
    }
}
