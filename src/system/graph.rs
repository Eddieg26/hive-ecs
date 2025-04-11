use super::System;

pub struct SystemExecutionState {
    dependency_count: Vec<usize>,
    exclusive: bool,
    non_send: bool,
}

pub struct SystemGraph {
    systems: Vec<System>,
    initial_systems: Box<[usize]>,
    dependencies: Box<[usize]>,
    dependents: Box<[Box<[usize]>]>,
}
