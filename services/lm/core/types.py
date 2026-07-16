from dataclasses import dataclass


@dataclass
class ChatMessage:
    role: str
    content: str


@dataclass
class GenerateResult:
    text: str
    tokens_processed: int = 0
    tokens_generated: int = 0
    duration_ms: float = 0.0


@dataclass
class HealthInfo:
    is_ready: bool
    model_name: str = ""
    context_length: int = 0
