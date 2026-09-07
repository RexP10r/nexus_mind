use crate::model::AgentAction;

pub trait AgentResponse {
    fn action(&self) -> AgentAction;
    fn thought(&self) -> String;
}

