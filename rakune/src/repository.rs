use std::fs::File;
use std::io::Write;
use std::process::Command;

use regex::Regex;

use crate::Diff;
use crate::Result;

#[derive(Clone, Default)]
pub struct GitRepository;

impl GitRepository {
    /// Edit the state of a respository using a given agent capability
    pub fn transform(&mut self, transformation: &Transformation) -> Result<()> {
        Ok(match transformation {
            Transformation::UpdateFragment {
                fragment,
                updated_lines,
            } => {
                let content = fragment.read_file()?;
                let mut lines = content.lines().collect::<Vec<_>>();

                if [fragment.line_range.0, fragment.line_range.1]
                    .iter()
                    .any(|r| !(0..=lines.len()).contains(r))
                {
                    let error_message = format!(
                        "One of the line ranges {:?} was not in bound of the file [0..{}].",
                        fragment.line_range,
                        lines.len(),
                    );
                    return Err(error_message.into());
                }

                lines.splice(
                    fragment.line_range.0..=fragment.line_range.1,
                    updated_lines.into_iter().map(String::as_str),
                );

                let mut file = File::options()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&fragment.filepath)?;

                file.write_all(&lines.join("\n").as_bytes())?;
            }
            _ => unreachable!(),
        })
    }

    pub fn diff(&self, target: Option<&String>) -> Result<Diff> {
        let mut command = Command::new("git");
        let command = match target {
            Some(other) => command.args(&["diff", other]),
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

    /// Searches through git or conversation history for context on a particular code fragment
    ///
    /// X change built from Y context worked for scenario Z, and scenario A is similar to
    /// scenario Z, so it should also read Y context.
    pub fn temporal_context(&self, _fragment: &Fragment) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// Searches through symbolic, lexical, or etc information on a particular code fragment
    /// such as callee/caller functions, classes, etc..
    #[allow(unreachable_code)]
    pub fn spatial_context(&self, fragment: &Fragment) -> Result<Vec<String>> {
        let context = vec![format!(
            "The existing lines of code are:\n\n{}\n>>>>\n{}\n<<<<",
            fragment.filepath,
            fragment
                .read_lines()?
                .lines()
                .into_iter()
                .enumerate()
                .map(|(i, s)| format!("{i} {s}"))
                .collect::<Vec<_>>()
                .join("\n"),
        )];

        return Ok(context);

        use tree_sitter::Parser;
        let mut parser = Parser::new();
        parser
            .set_language(tree_sitter_rust::language())
            .expect("Error loading Rust grammar");

        let source_code = fragment.read_file()?;

        let tree = parser
            .parse(source_code, None)
            .expect("Failed to parse tree.");
        let _root_node = tree.root_node();

        Ok(Vec::new())
    }
}

#[derive(Debug)]
pub struct Fragment {
    pub filepath: String,
    pub line_range: LineRange,
}

impl Fragment {
    pub fn read_file(&self) -> Result<String> {
        Ok(std::fs::read_to_string(&self.filepath)?)
    }

    pub fn read_lines(&self) -> Result<String> {
        let content = self.read_file()?;
        let lines = content.lines().collect::<Vec<_>>();

        if [self.line_range.0, self.line_range.1]
            .iter()
            .any(|r| !(0..=lines.len()).contains(r))
        {
            let error_message = format!(
                "One of the line ranges {:?} was not in bound of the file [0..{}].",
                self.line_range,
                lines.len(),
            );
            return Err(error_message.into());
        }

        Ok(lines[self.line_range.0..self.line_range.1].join("\n"))
    }
}

type LineRange = (usize, usize);

#[derive(Debug)]
pub struct Comment {
    pub message: String,
    pub fragments: Vec<Fragment>,
}

pub enum Transformation {
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
        fragment: Fragment,
        updated_lines: Vec<String>,
    },
    InsertFragment {
        line_no: usize,
        content: Vec<String>,
    },
}
impl TryFrom<&str> for Transformation {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        let re = Regex::new(
            "filepath: (.*?),?\n.*start_line: (\\d+),?\n.*end_line: (\\d+),?\n.*content: ([\\s\\S]*)```",
        )
        .expect("Regex failed to compile.");

        let transformation = re
            .captures_iter(value)
            .map(|c| c.extract())
            .map(|(_, [filepath, start, end, content])| {
                let start_line = start.parse().unwrap();
                let end_line = end.parse().unwrap();
                let updated_lines = content.lines().map(|s| s.to_string()).collect();

                Self::UpdateFragment {
                    fragment: Fragment {
                        filepath: filepath.into(),
                        line_range: (start_line, end_line),
                    },
                    updated_lines,
                }
            })
            .next()
            .ok_or_else(|| "failed to parse transformation".into());

        transformation
    }
}
