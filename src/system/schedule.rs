use super::{
    IntoSystemConfigs, SystemConfig,
    executor::{GraphInfo, RunMode, SystemExecutor},
};
use crate::world::{World, WorldCell};
use std::collections::{HashMap, HashSet};

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
}

impl PhaseConfig {
    pub fn new(phase: impl Phase) -> Self {
        Self {
            phase: Box::new(phase),
            configs: vec![],
        }
    }

    pub fn add_systems<M>(&mut self, systems: impl IntoSystemConfigs<M>) {
        self.configs.extend(systems.configs().flatten());
    }

    pub fn build(self, world: &mut World, mode: RunMode) -> PhaseNode {
        let info = GraphInfo::new(world, self.configs);
        let executor = mode.create_executor(info);

        PhaseNode {
            phase: self.phase,
            executor,
            children: vec![],
        }
    }
}

pub struct PhaseNode {
    phase: Box<dyn Phase>,
    executor: Box<dyn SystemExecutor>,
    children: Vec<PhaseNode>,
}

impl PhaseNode {
    pub fn execute(&self, world: WorldCell) {
        let ctx = PhaseContext::new(world, self.executor.as_ref());
        self.phase.run(ctx);

        for child in &self.children {
            child.execute(world);
        }
    }
}

pub struct Schedule {
    mode: RunMode,
    phases: Vec<PhaseConfig>,
    map: HashMap<&'static str, usize>,
    dependencies: HashMap<usize, HashSet<usize>>,
    children: HashMap<usize, HashSet<usize>>,
}

impl Schedule {
    pub fn new(mode: RunMode) -> Self {
        Self {
            mode,
            phases: vec![],
            map: HashMap::new(),
            dependencies: HashMap::new(),
            children: HashMap::new(),
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
                let index = self.phases.len();
                self.map.insert(phase.name(), index);
                self.phases.push(PhaseConfig::new(phase));
                index
            }
        }
    }

    pub fn add_sub_phase(&mut self, main: impl Phase, sub: impl Phase) {
        let main_index = self.add_phase(main);
        let sub_index = self.add_phase(sub);

        self.children
            .entry(main_index)
            .or_default()
            .insert(sub_index);
    }

    pub fn run_before(&mut self, phase: impl Phase, target: impl Phase) {
        let index = self.add_phase(phase);
        let target_index = self.add_phase(target);

        self.dependencies
            .entry(target_index)
            .or_default()
            .insert(index);
    }

    pub fn run_after(&mut self, phase: impl Phase, target: impl Phase) {
        self.run_before(target, phase);
    }

    pub fn add_systems<M>(&mut self, phase: impl Phase, systems: impl IntoSystemConfigs<M>) {
        let index = self.add_phase(phase);
        self.phases[index].add_systems(systems);
    }

    pub fn build(self, world: &mut World) -> Systems {
        todo!()
    }
}

pub struct Systems {
    mode: RunMode,
    phases: HashMap<&'static str, PhaseNode>,
}

impl Systems {
    pub fn mode(&self) -> RunMode {
        self.mode
    }

    pub fn run(&self, world: &mut World, phase: impl Phase) {
        if let Some(node) = self.phases.get(phase.name()) {
            let world = WorldCell::new_mut(world);

            node.execute(world);
        };
    }
}
