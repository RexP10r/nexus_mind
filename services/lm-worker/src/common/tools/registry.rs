use std::collections::HashMap;

use crate::common::traits::tool::Tool;

pub struct ToolRegistry<T: Tool> {
    tools: HashMap<String, Box<T>>,
}

impl<T: Tool> ToolRegistry<T> {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn from_tools(tools: Vec<Box<T>>) -> Self {
        let mut registry = Self::new();
        for tool in tools {
            registry.register(tool);
        }
        registry
    }

    pub fn register(&mut self, tool: Box<T>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn execute(&self, name: &str, input: &str) -> Option<String> {
        self.tools.get(name).map(|tool| tool.execute(input))
    }

    pub fn tool_descriptions(&self) -> String {
        self.tools
            .iter()
            .map(|(name, tool)| format!("- {}: {}", name, tool.description()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl<T: Tool> Default for ToolRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}
