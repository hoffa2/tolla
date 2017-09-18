use std::fs::File;

pub struct Audit;

impl Audit {
    fn new() -> Audit {
        Audit {
        }
    }
}

impl Policy for Audit {
}
