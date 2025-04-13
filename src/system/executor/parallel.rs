use crate::{system::System, world::WorldCell};
use fixedbitset::FixedBitSet;
use std::{
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{Sender, channel},
    },
    thread::Scope,
};

pub struct ParallelExecutor {
    state: Arc<Mutex<ExecutionState>>,
    dependencies: Box<[usize]>,
    systems: Box<[System]>,
}

impl ParallelExecutor {
    pub fn run(&self, world: WorldCell) {
        let (sender, receiver) = channel::<ExecutionResult>();

        let mut state = self.state.lock().unwrap();
        state.dependencies = self.dependencies.to_vec();
        drop(state);

        std::thread::scope(|scope| {
            let ctx = Arc::new(ExecutionContext::new(
                world,
                &self.systems,
                scope,
                &sender,
                self.state.clone(),
            ));

            ctx.execute();

            for result in receiver.iter() {
                match result {
                    ExecutionResult::Run(index) => ctx.run_system(index),
                    ExecutionResult::Done => break,
                }
            }
        });
    }
}

pub struct ExecutionState {
    dependents: Vec<FixedBitSet>,
    dependencies: Vec<usize>,
    initial_systems: FixedBitSet,
    queue: FixedBitSet,
    completed: FixedBitSet,
}

pub enum ExecutionResult {
    Run(usize),
    Done,
}

pub struct ExecutionContext<'scope, 'env: 'scope> {
    world: WorldCell<'scope>,
    systems: &'scope [System],
    scope: &'scope Scope<'scope, 'env>,
    sender: &'env Sender<ExecutionResult>,
    state: Arc<Mutex<ExecutionState>>,
}

impl<'scope, 'env: 'scope> ExecutionContext<'scope, 'env> {
    pub fn new(
        world: WorldCell<'scope>,
        systems: &'scope [System],
        scope: &'scope Scope<'scope, 'env>,
        sender: &'env Sender<ExecutionResult>,
        state: Arc<Mutex<ExecutionState>>,
    ) -> Self {
        Self {
            world,
            systems,
            scope,
            sender,
            state,
        }
    }

    fn execute(&self) {
        let mut state = self.state.lock().unwrap();
        state.queue = state.initial_systems.clone();

        self.spawn_systems(state);
    }

    fn scoped(&self) -> Self {
        let world = self.world;
        let systems = self.systems;
        let scope = self.scope;
        let sender = self.sender;
        let state = self.state.clone();

        Self {
            world,
            systems,
            scope,
            sender,
            state,
        }
    }

    fn spawn(&self, index: usize) {
        let scoped = self.scoped();
        scoped.scope.spawn(move || scoped.run_system(index));
    }

    fn spawn_non_send(&self, index: usize) {
        self.sender.send(ExecutionResult::Run(index)).unwrap();
    }

    fn spawn_systems(&self, mut state: MutexGuard<'_, ExecutionState>) {
        if state.completed.is_full() {
            let _ = self.sender.send(ExecutionResult::Done);
            return;
        }

        for index in state.queue.clone().into_ones() {
            state.queue.set(index, false);
            if self.systems[index].meta.send {
                self.spawn(index);
            } else {
                self.spawn_non_send(index);
            }
        }
    }

    fn run_system(&self, index: usize) {
        self.systems[index].execute(self.world);
        self.system_done(index);
    }

    fn system_done(&self, index: usize) {
        let mut state = self.state.lock().unwrap();

        state.completed.set(index, true);

        let dependents = state.dependents[index].clone();
        for dependent in dependents.ones() {
            state.dependencies[dependent] -= 1;
            if state.dependencies[dependent] == 0 {
                state.queue.set(dependent, true);
            }
        }

        self.spawn_systems(state);
    }
}
