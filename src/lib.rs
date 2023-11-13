use std::error::Error;
use std::result::Result as Res;

mod test;
pub mod llm;
pub mod repository;

pub(crate) type Result<T> = Res<T, Box<dyn Error>>;

type Prompt = str;
pub type Diff = String;

/// steps are meant to be limited context units of work
pub type Step = String;

pub trait Planner {
    ///
    fn completed(&self) -> bool;
    ///
    fn next_pending(&self) -> Step;
    ///
    fn from_prompt(&self, p: &Prompt) -> Self
    where
        Self: Sized;
}

/// data source abstraction.
/// anything that can be queried
pub trait DataSource<Q, A> {
    fn query(&self, query: &Q) -> Result<A>;
}

