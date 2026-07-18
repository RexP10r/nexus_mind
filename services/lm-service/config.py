from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_prefix="", case_sensitive=False)

    # Server settings
    host: str = Field(default="[::]", alias="HOST")
    port: int = Field(default=50051, alias="PORT")
    max_workers: int = Field(default=4, alias="MAX_WORKERS")

    # Model settings
    model_name: str = Field(
        default="./models/omnicoder-9b-q4_k_m.gguf", alias="MODEL_NAME")
    device: str = Field(default="cuda", alias="DEVICE")
    torch_dtype: str = Field(default="auto", alias="TORCH_DTYPE")

    # LLaMA.cpp params
    n_gpu_layers: int = Field(default=-1, alias="LLAMA_N_GPU_LAYERS")
    n_ctx: int = Field(default=16384, alias="LLAMA_N_CTX")
    n_batch: int = Field(default=512, alias="LLAMA_N_BATCH")
    n_ubatch: int = Field(default=256, alias="LLAMA_N_UBATCH")
    n_threads: int | None = Field(default=None, alias="LLAMA_N_THREADS")
    n_threads_batch: int | None = Field(
        default=None, alias="LLAMA_N_THREADS_BATCH")

    # Memory & Performance
    offload_kqv: bool = Field(default=False, alias="LLAMA_OFFLOAD_KQV")
    flash_attn: bool = Field(default=False, alias="LLAMA_FLASH_ATTN")
    low_vram: bool = Field(default=False, alias="LLAMA_LOW_VRAM")

    # Quantization
    n_parts: int | None = Field(default=None, alias="LLAMA_N_PARTS")
    use_mmap: bool = Field(default=True, alias="LLAMA_USE_MMAP")
    use_mlock: bool = Field(default=False, alias="LLAMA_USE_MLOCK")
