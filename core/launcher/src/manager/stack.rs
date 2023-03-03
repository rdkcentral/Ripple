use ripple_sdk::log::debug;

#[derive(Default, Debug, Clone)]
pub struct Stack {
    stack: Vec<String>,
}

impl Stack {
    pub fn new() -> Self {
        Stack { stack: Vec::new() }
    }

    pub fn peek(&self) -> Option<&String> {
        self.stack.last()
    }

    pub fn push(&mut self, item: String) {
        self.stack.push(item);
    }

    pub fn pop_item(&mut self, item: &str) {
        self.stack.retain(|name| name.ne(&item));
    }

    pub fn contains(&mut self, item: &String) -> bool {
        self.stack.contains(item)
    }

    pub fn bring_to_front(&mut self, item: &str) {
        self.pop_item(item);
        self.push(item.to_string());
    }

    pub fn send_to_back(&mut self, item: &str) {
        self.pop_item(item);
        self.stack.insert(0, item.to_string());
    }

    pub fn dump_stack(&mut self) {
        debug!("dump_stack: {:?}", self.stack);
    }

    pub fn len(&mut self) -> usize {
        self.stack.len()
    }
}
