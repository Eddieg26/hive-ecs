use super::SystemExecutor;
use crate::{
    core::{ImmutableIndexDag, IndexDag},
    system::System,
    world::WorldCell,
};
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
    systems: ImmutableIndexDag<System>,
    initial_systems: FixedBitSet,
}

impl ParallelExecutor {
    pub fn new(systems: IndexDag<System>) -> Self {
        let systems = systems.into_immutable();

        let mut initial_systems = FixedBitSet::with_capacity(systems.len());
        for (index, deps) in systems.dependencies().iter().enumerate() {
            initial_systems.set(index, *deps == 0);
        }

        let state = ExecutionState {
            dependencies: systems.dependencies().to_vec(),
            queue: initial_systems.clone(),
            completed: FixedBitSet::with_capacity(systems.len()),
        };

        Self {
            state: Arc::new(Mutex::new(state)),
            systems,
            initial_systems,
        }
    }

    fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        state.completed.clear();
        state.queue = self.initial_systems.clone();
        state.dependencies = self.systems.dependencies().to_vec();
    }
}

impl SystemExecutor for ParallelExecutor {
    fn execute(&self, world: WorldCell) {
        let (sender, receiver) = channel::<ExecutionResult>();

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

        self.reset();
    }
}

pub struct ExecutionState {
    dependencies: Vec<usize>,
    queue: FixedBitSet,
    completed: FixedBitSet,
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self {
            dependencies: Default::default(),
            queue: Default::default(),
            completed: Default::default(),
        }
    }
}

pub enum ExecutionResult {
    Run(usize),
    Done,
}

pub struct ExecutionContext<'scope, 'env: 'scope> {
    world: WorldCell<'scope>,
    systems: &'scope ImmutableIndexDag<System>,
    scope: &'scope Scope<'scope, 'env>,
    sender: &'env Sender<ExecutionResult>,
    state: Arc<Mutex<ExecutionState>>,
}

impl<'scope, 'env: 'scope> ExecutionContext<'scope, 'env> {
    pub fn new(
        world: WorldCell<'scope>,
        systems: &'scope ImmutableIndexDag<System>,
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
        let state = self.state.lock().unwrap();
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
            if self.systems.nodes()[index].meta.send {
                self.spawn(index);
            } else {
                self.spawn_non_send(index);
            }
        }
    }

    fn run_system(&self, index: usize) {
        self.systems.nodes()[index].run(self.world);
        self.system_done(index);
    }

    fn system_done(&self, index: usize) {
        let mut state = self.state.lock().unwrap();

        state.completed.set(index, true);

        for dependent in self.systems.dependents()[index].ones() {
            state.dependencies[dependent] -= 1;
            if state.dependencies[dependent] == 0 {
                state.queue.set(dependent, true);
            }
        }

        self.spawn_systems(state);
    }
}
