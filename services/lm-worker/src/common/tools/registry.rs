use crate::common::traits::tool::Tool;
use std::collections::HashMap;

pub trait ToolRegistry: Send + Sync {
    fn execute(&self, name: &str, input: &str) -> Option<String>;
    fn descriptions(&self) -> String;
}

pub struct InMemoryToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl InMemoryToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn from_tools(tools: Vec<Box<dyn Tool>>) -> Self {
        let mut registry = Self::new();
        for tool in tools {
            registry.register(tool);
        }
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }
}

impl Default for InMemoryToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry for InMemoryToolRegistry {
    fn execute(&self, name: &str, input: &str) -> Option<String> {
        self.tools.get(name).map(|tool| tool.execute(input))
    }

    fn descriptions(&self) -> String {
        self.tools
            .iter()
            .map(|(name, tool)| format!("- {}: {}", name, tool.description()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
