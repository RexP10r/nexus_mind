from dataclasses import dataclass


@dataclass
class Settings:
    model_name: str = "mistralai/Mistral-7B-Instruct-v0.1"
    device: str = "cpu"
    torch_dtype: str = "auto"
    host: str = "[::]"
    port: int = 50051
    max_workers: int = 4

    @classmethod
    def from_env(cls) -> "Settings":
        import os

        return cls(
            model_name=os.getenv("MODEL_NAME", cls.model_name),
            device=os.getenv("DEVICE", cls.device),
            torch_dtype=os.getenv("TORCH_DTYPE", cls.torch_dtype),
            host=os.getenv("HOST", cls.host),
            port=int(os.getenv("PORT", str(cls.port))),
            max_workers=int(os.getenv("MAX_WORKERS", str(cls.max_workers))),
        )
