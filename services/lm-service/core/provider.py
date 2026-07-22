from abc import ABC, abstractmethod

from core.types import ChatMessage, GenerateResult, HealthInfo


class LMProvider(ABC):
    @abstractmethod
    def load_model(self) -> None:
        pass

    @abstractmethod
    def generate(
        self,
        messages: list[ChatMessage],
        temperature: float = 0.7,
        max_tokens: int = 256,
        top_p: float | None = None,
        top_k: int | None = None,
    ) -> GenerateResult:
        pass

    @abstractmethod
    def health_check(self) -> HealthInfo:
        pass

    def close(self) -> None:
        pass

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()
