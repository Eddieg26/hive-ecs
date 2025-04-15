use super::{
    IntoSystemConfigs, SystemConfig,
    executor::{GraphInfo, RunMode, SystemExecutor},
};
use crate::{
    core::{ImmutableIndexDag, IndexDag},
    world::{World, WorldCell},
};
use std::collections::HashMap;

pub struct PhaseContext<'a> {
    world: WorldCell<'a>,
    executor: &'a dyn SystemExecutor,
}

impl<'a> PhaseContext<'a> {
    pub(crate) fn new(world: WorldCell<'a>, executor: &'a dyn SystemExecutor) -> Self {
        Self { world, executor }
    }

    pub unsafe fn world(&self) -> WorldCell {
        self.world
    }

    pub fn execute(&self) {
        self.executor.execute(self.world);
    }
}

pub trait Phase: 'static {
    fn run(&self, ctx: PhaseContext) {
        ctx.execute();
    }

    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub struct PhaseConfig {
    phase: Box<dyn Phase>,
    configs: Vec<SystemConfig>,
    parent: Option<usize>,
}

impl PhaseConfig {
    pub fn new(phase: impl Phase) -> Self {
        Self {
            phase: Box::new(phase),
            configs: vec![],
            parent: None,
        }
    }

    pub fn add_systems<M>(&mut self, systems: impl IntoSystemConfigs<M>) {
        self.configs.extend(systems.configs().flatten());
    }

    pub fn set_parent(&mut self, index: usize) {
        self.parent = Some(index)
    }

    pub fn build(self, world: &mut World, mode: RunMode) -> PhaseNode {
        let info = GraphInfo::new(world, self.configs);
        let executor = mode.create_executor(info);

        PhaseNode {
            phase: self.phase,
            executor,
        }
    }
}

pub struct PhaseNode {
    phase: Box<dyn Phase>,
    executor: Box<dyn SystemExecutor>,
}

impl PhaseNode {
    pub fn run(&self, world: WorldCell) {
        let ctx = PhaseContext::new(world, self.executor.as_ref());
        self.phase.run(ctx);
    }
}

pub struct Schedule {
    mode: RunMode,
    phases: IndexDag<PhaseConfig>,
    hierarchy: IndexDag<usize>,
    map: HashMap<&'static str, usize>,
}

impl Schedule {
    pub fn new(mode: RunMode) -> Self {
        Self {
            mode,
            phases: IndexDag::new(),
            hierarchy: IndexDag::new(),
            map: HashMap::new(),
        }
    }

    pub fn mode(&self) -> RunMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: RunMode) {
        self.mode = mode;
    }

    pub fn add_phase(&mut self, phase: impl Phase) -> usize {
        match self.map.get(phase.name()).copied() {
            Some(index) => index,
            None => {
                let config = PhaseConfig::new(phase);
                let index = self.phases.add_node(config);
                self.map
                    .insert(self.phases.nodes()[index].phase.name(), index);
                self.hierarchy.add_node(index);
                index
            }
        }
    }

    pub fn add_sub_phase(&mut self, main: impl Phase, sub: impl Phase) {
        let main_index = self.add_phase(main);
        let sub_index = self.add_phase(sub);

        self.hierarchy.add_dependency(main_index, sub_index);
    }

    pub fn run_before(&mut self, phase: impl Phase, target: impl Phase) {
        let index = self.add_phase(phase);
        let target_index = self.add_phase(target);

        self.phases.add_dependency(index, target_index);

        if let Some(old_parent) = self.phases.nodes()[index].parent {
            self.hierarchy.remove_dependency(old_parent, index);
        }

        self.phases.nodes_mut()[index].parent = self.phases.nodes()[target_index].parent;
        if let Some(parent) = self.phases.nodes()[index].parent {
            self.hierarchy.add_dependency(parent, index);
        }
    }

    pub fn run_after(&mut self, phase: impl Phase, target: impl Phase) {
        self.run_before(target, phase);
    }

    pub fn add_systems<M>(&mut self, phase: impl Phase, systems: impl IntoSystemConfigs<M>) {
        let index = self.add_phase(phase);
        self.phases.nodes_mut()[index].add_systems(systems);
    }

    pub fn build(self, world: &mut World) -> Result<Systems, ScheduleBuildError> {
        let mode = self.mode;
        let mut hierarchy = self.hierarchy;
        let mut phases = self.phases.map(|config| config.build(world, mode));

        if let Err(error) = hierarchy.build() {
            let names = error
                .0
                .iter()
                .map(|index| phases.nodes()[*index].phase.name())
                .collect();
            return Err(ScheduleBuildError::CyclicHierarchy(names));
        }

        if let Err(error) = phases.build() {
            let names = error
                .0
                .iter()
                .map(|index| phases.nodes()[*index].phase.name())
                .collect();
            return Err(ScheduleBuildError::CyclicHierarchy(names));
        }

        let systems = Systems {
            mode,
            phases: phases.into_immutable(),
            hierarchy: hierarchy.into_immutable(),
            map: self.map,
        };

        Ok(systems)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleBuildError {
    CyclicDependency(Vec<&'static str>),
    CyclicHierarchy(Vec<&'static str>),
}

impl std::fmt::Display for ScheduleBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleBuildError::CyclicDependency(names) => {
                write!(f, "Cyclic dependency detected: {:?}", names)
            }
            ScheduleBuildError::CyclicHierarchy(names) => {
                write!(f, "Cyclic hierarchy detected: {:?}", names)
            }
        }
    }
}

pub struct Systems {
    mode: RunMode,
    phases: ImmutableIndexDag<PhaseNode>,
    hierarchy: ImmutableIndexDag<usize>,
    map: HashMap<&'static str, usize>,
}

impl Systems {
    pub fn mode(&self) -> RunMode {
        self.mode
    }

    pub fn run(&self, world: &mut World, phase: impl Phase) {
        if let Some(index) = self.map.get(phase.name()).copied() {
            let world = unsafe { WorldCell::new_mut(world) };

            let mut stack = vec![index];
            while let Some(index) = stack.pop() {
                self.phases.nodes()[index].run(world);
                stack.extend(self.hierarchy.dependents()[index].ones());
            }
        }
    }
}
