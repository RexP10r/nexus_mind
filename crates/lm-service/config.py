from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict
from pathlib import Path


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix="",
        case_sensitive=False,
        env_file=str(Path(__file__).resolve().parents[2] / ".env"),
        extra="ignore",
    )

    grpc_address: str = Field(alias="LM_SERVICE_GRPC_ADDR_CUTTED")
    max_workers: int = Field(alias="LM_SERVICE_MAX_WORKERS")

    model_name: str = Field(alias="MODEL_NAME")
    device: str = Field(alias="DEVICE")
    torch_dtype: str = Field(alias="TORCH_DTYPE")

    n_gpu_layers: int = Field(alias="LLAMA_N_GPU_LAYERS")
    n_ctx: int = Field(alias="LLAMA_N_CTX")
    n_batch: int = Field(alias="LLAMA_N_BATCH")
    n_ubatch: int = Field(alias="LLAMA_N_UBATCH")
    n_threads: int | None = Field(default=None, alias="LLAMA_N_THREADS")
    n_threads_batch: int | None = Field(default=None, alias="LLAMA_N_THREADS_BATCH")
    chat_format: str | None = Field(default=None, alias="LLAMA_CHAT_FORMAT")

    offload_kqv: bool = Field(alias="LLAMA_OFFLOAD_KQV")
    flash_attn: bool = Field(alias="LLAMA_FLASH_ATTN")
    low_vram: bool = Field(alias="LLAMA_LOW_VRAM")

    n_parts: int | None = Field(default=None, alias="LLAMA_N_PARTS")
    use_mmap: bool = Field(alias="LLAMA_USE_MMAP")
    use_mlock: bool = Field(alias="LLAMA_USE_MLOCK")
