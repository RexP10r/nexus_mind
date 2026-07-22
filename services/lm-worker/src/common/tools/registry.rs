use std::collections::HashMap;

use crate::common::traits::tool::Tool;

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
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

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
