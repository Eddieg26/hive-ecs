use core::Frame;

use app::App;
use system::{
    query::{Added, Query},
    schedule::Phase,
};
use world::{
    Command, CommandBuffer, Component, Event, EventReader, EventWriter, Resource, Resources,
    Spawner, World,
};

pub mod app;
pub mod core;
pub mod system;
pub mod world;

fn main() {
    // App::new()
    //     .register::<Name>()
    //     .register::<Age>()
    //     .add_systems(Start, |mut events: EventWriter<TestEvent>| {
    //         events.send(TestEvent);
    //     })
    //     .add_systems(Update, |events: EventReader<TestEvent>| {
    //         for event in events {
    //             println!("{:?}", event);
    //         }
    //     })
    //     .build()
    //     .run(Start)
    //     .run(Update);

    let mut resources = Resources::new();
    let age = resources.add::<true, _>(Age(30), Frame::ZERO);
    let name = resources.add::<true, _>(Name("John"), Frame::ZERO);

    let age = resources.get::<Age>(age).unwrap();
    println!("{:?}", age);

    let name = resources.get::<Name>(name).unwrap();
    println!("{:?}", name);
}

pub struct Start;
impl Phase for Start {}

pub struct Update;
impl Phase for Update {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestEvent;
impl Event for TestEvent {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Age(u32);
impl Component for Age {}
impl Resource for Age {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(&'static str);
impl Component for Name {}
impl Resource for Name {}

impl Command for Age {
    fn execute(self, world: &mut world::World) {
        println!("{:?}", self)
    }
}

impl Command for Name {
    fn execute(self, world: &mut world::World) {
        println!("{:?}", self)
    }
}

#[derive(Debug)]
pub struct Names(Vec<&'static str>);

impl Command for Names {
    fn execute(self, world: &mut World) {
        println!("{:?}", self)
    }
}
