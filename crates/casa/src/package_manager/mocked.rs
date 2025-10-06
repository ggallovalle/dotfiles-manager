use std::{cell::Cell, process::Command};

use crate::package_manager::CommandRunner;

pub struct MockedCommandRunner {
    responses: Vec<(i32, String, String)>,
    call_count: Cell<usize>,
}

impl MockedCommandRunner {
    pub fn new(responses: Vec<(i32, String, String)>) -> Self {
        Self { responses, call_count: Cell::new(0) }
    }

    pub fn add_success(&mut self, stdout: &str) {
        self.responses.push((0, stdout.to_string(), "".to_string()));
    }

    pub fn add_failure(&mut self, stderr: &str) {
        self.responses.push((1, "".to_string(), stderr.to_string()));
    }

    pub fn add_warning(&mut self, stderr: &str) {
        self.responses.push((0, "".to_string(), stderr.to_string()));
    }

    pub fn add(&mut self, code: i32, stdout: &str, stderr: &str) {
        self.responses.push((code, stdout.to_string(), stderr.to_string()));
    }
}

impl Default for MockedCommandRunner {
    fn default() -> Self {
        return Self { call_count: Cell::new(0), responses: vec![] };
    }
}

impl CommandRunner for MockedCommandRunner {
    fn execute(&self, _command: Command) -> (i32, String, String) {
        let count = self.call_count.get();
        if !self.responses.is_empty() && count < self.responses.len() {
            let response = self.responses[count].clone();
            self.call_count.set(count + 1);
            response
        } else {
            (1, "".to_string(), "No more mocked responses".to_string())
        }
    }
}
