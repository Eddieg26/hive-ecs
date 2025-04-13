use std::collections::VecDeque;

pub type ScopedTask<'a> = Box<dyn FnOnce() + Send + 'a>;

pub struct ScopedTaskPool<'a> {
    size: usize,
    queue: VecDeque<ScopedTask<'a>>,
}

impl<'a> ScopedTaskPool<'a> {
    pub fn new(size: usize) -> Self {
        ScopedTaskPool {
            size,
            queue: VecDeque::new(),
        }
    }

    pub fn spawn(&mut self, task: impl FnOnce() + Send + 'a) -> &mut Self {
        self.queue.push_back(Box::new(task));

        self
    }

    pub fn run(&mut self) {
        while !self.queue.is_empty() {
            let len = self.queue.len().min(self.size);
            let tasks = self.queue.drain(..len).collect::<Vec<_>>();
            std::thread::scope(move |scope| {
                for task in tasks {
                    scope.spawn(move || task());
                }
            });
        }
    }
}

pub fn max_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}
