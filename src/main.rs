use core::{
    Component, ComponentId, Entity,
    blob::Blob,
    table::{Column, Row, TableBuilder, TableCell},
};
use world::query::{Added, Modified, Query, QueryState};

pub mod core;
pub mod world;

fn main() {
    let mut world = world::World::new();
    world.register::<Name>();
    world.register::<Age>();

    let entity = world.spawn();
    world.add_component(entity, Age(30));
    world.add_component(entity, Name("Alice".to_string()));

    let state = QueryState::new(&world);
    let query = Query::<(&Age, &Name), Added<Age>>::new(&world, &state);
    for (age, name) in query.iter() {
        println!("Query found age: {:?}", age);
        println!("Query found name: {:?}", name);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Age(u32);
impl Component for Age {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(String);
impl Component for Name {}
