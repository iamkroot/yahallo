/// Helper to time various parts of the code.
/// Timer stops and is printed when struct is dropped.
pub(crate) struct Stopwatch {
    start: std::time::Instant,
    name: &'static str,
}

impl Stopwatch {
    pub(crate) fn new(name: &'static str) -> Self {
        Self {
            name,
            start: std::time::Instant::now(),
        }
    }
    /// Time a closure, returning the result.
    pub(crate) fn time<T, F: FnOnce() -> T>(name: &'static str, func: F) -> T {
        let _sw = Self::new(name);
        func()
    }
}

impl Drop for Stopwatch {
    fn drop(&mut self) {
        println!(
            "[{}] elapsed {}ms",
            self.name,
            self.start.elapsed().as_millis()
        );
    }
}
