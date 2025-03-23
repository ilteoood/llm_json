


#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum ContextValues {
    ObjectKey,
    ObjectValue,
    Array,
}

pub struct JsonContext {
    pub context: Vec<ContextValues>,
    pub current: Option<ContextValues>,
    pub empty: bool,
}

impl JsonContext {
    pub fn new() -> Self {
        JsonContext {
            context: Vec::new(),
            current: None,
            empty: true,
        }
    }

    pub fn set(&mut self, value: ContextValues) {
        self.context.push(value);
        self.current = Some(value);
        self.empty = false;
    }

    pub fn reset(&mut self) {
        match self.context.pop() {
            Some(value) => {
                self.current = Some(value);
                self.empty = false;
            }
            None => {
                self.current = None;
                self.empty = true;
            }
        };
    }
}
