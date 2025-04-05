pub trait Event: Sized + 'static {}

pub struct Events<E: Event> {
    write: Vec<E>,
    read: Vec<E>,
}

impl<E: Event> Events<E> {
    pub(crate) fn new() -> Self {
        Self {
            write: Vec::new(),
            read: Vec::new(),
        }
    }

    pub(crate) fn update(&mut self) {
        self.read = std::mem::take(&mut self.write);
    }
}

pub struct EventReader<'a, E: Event> {
    events: &'a Events<E>,
    index: usize,
}

impl<'a, E: Event> EventReader<'a, E> {
    pub(crate) fn new(events: &'a Events<E>) -> Self {
        Self { events, index: 0 }
    }
}

impl<'a, E: Event> Iterator for EventReader<'a, E> {
    type Item = &'a E;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.events.read.len() {
            let event = &self.events.read[self.index];
            self.index += 1;
            Some(event)
        } else {
            None
        }
    }
}

impl<'a, E: Event> IntoIterator for &'a Events<E> {
    type Item = &'a E;
    type IntoIter = EventReader<'a, E>;

    fn into_iter(self) -> Self::IntoIter {
        EventReader::new(self)
    }
}

pub struct EventWriter<'a, E: Event> {
    events: &'a mut Events<E>,
}

impl<'a, E: Event> EventWriter<'a, E> {
    pub(crate) fn new(events: &'a mut Events<E>) -> Self {
        Self { events }
    }

    pub fn send(&mut self, event: E) {
        self.events.write.push(event);
    }
}
