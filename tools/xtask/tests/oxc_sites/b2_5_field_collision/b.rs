// B2-5: the other `Ctx` (see a.rs). Its field `inner` is a `Plain`, whose `step` calls nothing.

pub struct Ctx {
    inner: Plain,
}

pub struct Plain;

impl Plain {
    fn step(&mut self) -> u32 {
        0
    }
}
