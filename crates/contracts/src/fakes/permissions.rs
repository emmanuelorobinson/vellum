//! A permission prompt that answers from a script, standing in for the Permission card and the terminal.
use std::cell::RefCell;
use std::collections::VecDeque;
use vellum_core::ports::{Answer, EngineFuture, PermissionPrompt, Question};

/// Answers each question with the next scripted answer (`None` = dismissed). Once the script runs out it keeps
/// giving `fallback`. Records every question it was asked.
#[derive(Debug)]
pub struct ScriptedPrompt {
    answers: RefCell<VecDeque<Option<Answer>>>,
    fallback: Option<Answer>,
    asked: RefCell<Vec<Question>>,
}

impl ScriptedPrompt {
    pub fn new(
        answers: impl IntoIterator<Item = Option<Answer>>,
        fallback: Option<Answer>,
    ) -> Self {
        Self {
            answers: RefCell::new(answers.into_iter().collect()),
            fallback,
            asked: RefCell::default(),
        }
    }
    /// Always answers `answer`.
    pub fn always(answer: Answer) -> Self {
        Self::new([], Some(answer))
    }
    /// The questions so far, oldest first.
    pub fn asked(&self) -> Vec<Question> {
        self.asked.borrow().clone()
    }
}

impl PermissionPrompt for ScriptedPrompt {
    fn ask<'a>(&'a self, question: Question) -> EngineFuture<'a, Option<Answer>> {
        self.asked.borrow_mut().push(question);
        let answer = self
            .answers
            .borrow_mut()
            .pop_front()
            .unwrap_or(self.fallback);
        Box::pin(async move { answer })
    }
}
