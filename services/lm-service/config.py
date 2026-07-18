from dataclasses import dataclass, field


@dataclass
class Settings:
    # Server settings
    host: str = "[::]"
    port: int = 50051
    max_workers: int = 4

    # Model path and basic settings
    model_name: str = "./models/Qwen3.5-9B-Q4_K_M.gguf"
    device: str = "cuda"
    torch_dtype: str = "auto"

    # LLaMA.cpp loading parameters - all configurable via env vars
    n_gpu_layers: int = field(
        default=-1, metadata={"env": "LLAMA_N_GPU_LAYERS"})
    n_ctx: int = field(default=16384, metadata={"env": "LLAMA_N_CTX"})
    n_batch: int = field(default=512, metadata={"env": "LLAMA_N_BATCH"})
    n_ubatch: int = field(default=256, metadata={"env": "LLAMA_N_UBATCH"})
    n_threads: int = field(default=None, metadata={"env": "LLAMA_N_THREADS"})
    n_threads_batch: int = field(default=None, metadata={
                                 "env": "LLAMA_N_THREADS_BATCH"})

    # Memory and performance settings
    offload_kqv: bool = field(default=False, metadata={
                              "env": "LLAMA_OFFLOAD_KQV"})
    flash_attn: bool = field(default=True, metadata={
                             "env": "LLAMA_FLASH_ATTN"})
    low_vram: bool = field(default=False, metadata={"env": "LLAMA_LOW_VRAM"})

    # Quantization settings (if applicable)
    n_parts: int = field(default=None, metadata={"env": "LLAMA_N_PARTS"})
    use_mmap: bool = field(default=True, metadata={"env": "LLAMA_USE_MMAP"})
    use_mlock: bool = field(default=False, metadata={"env": "LLAMA_USE_MLOCK"})

    @classmethod
    def from_env(cls) -> "Settings":
        import os

        n_threads_str = os.getenv("LLAMA_N_THREADS")
        n_threads_batch_str = os.getenv("LLAMA_N_THREADS_BATCH")

        return cls(
            model_name=os.getenv("MODEL_NAME", cls.model_name),
            device=os.getenv("DEVICE", cls.device),
            torch_dtype=os.getenv("TORCH_DTYPE", cls.torch_dtype),

            # LLaMA.cpp params from env or defaults
            n_gpu_layers=int(
                os.getenv("LLAMA_N_GPU_LAYERS", str(cls.n_gpu_layers))),
            n_ctx=int(os.getenv("LLAMA_N_CTX", str(cls.n_ctx))),
            n_batch=int(os.getenv("LLAMA_N_BATCH", str(cls.n_batch))),
            n_ubatch=int(os.getenv("LLAMA_N_UBATCH", str(cls.n_ubatch))),

            # Handle None values for thread counts (auto-detect)
            n_threads=(int(n_threads_str) if n_threads_str else cls.n_threads),
            n_threads_batch=(int(n_threads_batch_str)
                             if n_threads_batch_str else cls.n_threads_batch),

            offload_kqv=os.getenv("LLAMA_OFFLOAD_KQV", str(
                cls.offload_kqv)).lower() == "true",
            flash_attn=os.getenv("LLAMA_FLASH_ATTN", str(
                cls.flash_attn)).lower() == "true",
            low_vram=os.getenv("LLAMA_LOW_VRAM", str(
                cls.low_vram)).lower() == "true",

            n_parts=(int(os.getenv("LLAMA_N_PARTS")) if os.getenv(
                "LLAMA_N_PARTS") else cls.n_parts),
            use_mmap=os.getenv("LLAMA_USE_MMAP", str(
                cls.use_mmap)).lower() == "true",
            use_mlock=os.getenv("LLAMA_USE_MLOCK", str(
                cls.use_mlock)).lower() == "true",

            host=os.getenv("HOST", cls.host),
            port=int(os.getenv("PORT", str(cls.port))),
            max_workers=int(os.getenv("MAX_WORKERS", str(cls.max_workers))),
        )
