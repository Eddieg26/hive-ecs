use std::sync::{
    Arc, Mutex,
    mpsc::{Sender, channel},
};

use super::System;
use crate::{core::FixedBitSet, world::WorldCell};

pub struct ExecutionState {
    dependents: Vec<Box<[usize]>>,
    dependencies: Vec<usize>,
    exclusive: bool,
    queue: FixedBitSet,
    completed: FixedBitSet,
}

pub enum ExecuteResult {
    Run(usize),
    Done,
    Error,
}

pub struct SystemGraph {
    systems: Vec<System>,
    initial_systems: FixedBitSet,
    dependencies: Box<[usize]>,
}

impl ExecutionState {
    fn start(state: Arc<Mutex<ExecutionState>>, systems: &SystemGraph, world: WorldCell) {
        let (sender, receiver) = channel::<(ExecuteResult, Arc<Sender<ExecuteResult>>)>();

        std::thread::scope(|s| {
            let exec_state = state.clone();
            s.spawn(move || {
                Self::run(exec_state, systems, world, sender);
            });

            for (result, sender) in receiver.iter() {
                match result {
                    ExecuteResult::Run(index) => {
                        let state = state.clone();
                        let system = &systems.systems[index];
                        Self::run_system(world, index, system, state, sender);
                    }
                    ExecuteResult::Done | ExecuteResult::Error => break,
                }
            }
        });
    }

    fn run(
        state: Arc<Mutex<ExecutionState>>,
        systems: &SystemGraph,
        world: WorldCell,
        global_sender: Sender<(ExecuteResult, Arc<Sender<ExecuteResult>>)>,
    ) {
        let (sender, receiver) = channel::<ExecuteResult>();
        let sender = Arc::new(sender);

        std::thread::scope(|s| {
            for index in systems.initial_systems.ones() {
                let exec_state = state.clone();
                if systems.systems[index].meta.send {
                    let sender = sender.clone();
                    s.spawn(move || {
                        let system = &systems.systems[index];
                        Self::run_system(world, index, system, exec_state, sender);
                    });
                } else {
                    global_sender
                        .send((ExecuteResult::Run(index), sender.clone()))
                        .unwrap();
                }
            }

            for result in receiver.iter() {
                match result {
                    ExecuteResult::Run(_) => {
                        let queue = {
                            let mut state = state.lock().unwrap();
                            let queue = state.queue.clone();
                            state.queue.clear();
                            queue
                        };

                        for index in queue.ones() {
                            let state = state.clone();
                            if systems.systems[index].meta.send {
                                let sender = sender.clone();
                                s.spawn(move || {
                                    let system = &systems.systems[index];
                                    Self::run_system(world, index, system, state, sender);
                                });
                            } else {
                                global_sender
                                    .send((ExecuteResult::Run(index), sender.clone()))
                                    .unwrap();
                            }
                        }
                    }
                    ExecuteResult::Done => {}
                    ExecuteResult::Error => continue,
                }
            }
        });
    }

    fn run_system(
        world: WorldCell,
        system_index: usize,
        system: &System,
        state: Arc<Mutex<ExecutionState>>,
        sender: Arc<Sender<ExecuteResult>>,
    ) {
        system.execute(world);

        let result = match state.lock() {
            Ok(mut execution_state) => {
                execution_state.completed.set(system_index, true);
                for dependent in execution_state.dependents[system_index].clone() {
                    execution_state.dependencies[dependent] -= 1;
                    if execution_state.dependencies[dependent] == 0 {
                        execution_state.queue.set(dependent, true);
                    }
                }

                match execution_state.completed.is_full() {
                    true => ExecuteResult::Done,
                    false => ExecuteResult::Run(0),
                }
            }
            Err(_) => ExecuteResult::Error,
        };

        let _ = sender.send(result);
    }
}
