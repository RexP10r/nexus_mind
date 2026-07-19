import time
import os

from llama_cpp import Llama

from core.provider import LMProvider
from core.types import ChatMessage, GenerateResult, HealthInfo

from config import Settings


class LlamaCppProvider(LMProvider):
    def __init__(self, settings: "Settings"):
        self._settings = settings

        self._model_name = settings.model_name
        self._n_gpu_layers = settings.n_gpu_layers
        self._n_ctx = settings.n_ctx
        self._n_batch = settings.n_batch
        self._n_ubatch = settings.n_ubatch
        self._n_threads = settings.n_threads
        self._n_threads_batch = settings.n_threads_batch
        self._offload_kqv = settings.offload_kqv
        self._flash_attn = settings.flash_attn
        self._low_vram = settings.low_vram
        self._use_mmap = settings.use_mmap
        self._use_mlock = settings.use_mlock
        self._chat_format = settings.chat_format

        self.load_model()

    def load_model(self):
        print(f"Loading GGUF model {self._model_name}...")

        kwargs = {
            "model_path": self._model_name,
            "n_gpu_layers": self._n_gpu_layers,
            "n_ctx": self._n_ctx,
            "n_batch": self._n_batch,
            "n_ubatch": self._n_ubatch,

            "n_threads": min(os.cpu_count() or 4, self._n_threads or 4),
            "n_threads_batch": self._n_threads_batch,

            "offload_kqv": self._offload_kqv,
            "flash_attn": self._flash_attn,
            "low_vram": self._low_vram,
            "use_mmap": self._use_mmap,
            "use_mlock": self._use_mlock,
        }

        if self._chat_format is not None:
            kwargs["chat_format"] = self._chat_format

        print(f"Loading with params: n_gpu_layers={self._n_gpu_layers}, n_ctx={
              self._n_ctx}, n_batch={self._n_batch}")

        self._model = Llama(**kwargs, verbose=False)

        print(f"Detected chat_format: {self._model.chat_format}")
        if "tokenizer.chat_template" in self._model.metadata:
            template = self._model.metadata["tokenizer.chat_template"]
            print(f"Chat template length: {len(template)} chars")
            print(f"Chat template (first 512 chars): {template[:512]}")

        health_info = self.health_check()
        if health_info.is_ready:
            print(f"Model loaded successfully. Context length: {
                  health_info.context_length}")

    def generate(self,
                 messages: list[ChatMessage],
                 temperature: float = 0.7,
                 max_tokens: int = 256,
                 top_p: float = 0.9,
                 top_k: int = 32,
                 ) -> GenerateResult:

        llm_messages = [{"role": m.role, "content": m.content}
                        for m in messages]

        start_time = time.perf_counter()
        response = self._model.create_chat_completion(
            messages=llm_messages,
            max_tokens=max_tokens,
            temperature=temperature,
            top_k=top_k,
            top_p=top_p,
            response_format={"type": "json_object"}
        )
        elapsed_ms = (time.perf_counter() - start_time) * 1000

        response_text = response["choices"][0]["message"]["content"]
        usage = response.get("usage", {})

        return GenerateResult(
            text=response_text,
            tokens_processed=usage.get("prompt_tokens", 0),
            tokens_generated=usage.get("completion_tokens", 0),
            duration_ms=elapsed_ms,
        )

    def health_check(self) -> HealthInfo:
        if self._model is None:
            return HealthInfo(is_ready=False)

        return HealthInfo(
            is_ready=True,
            model_name=self._model_name,
            context_length=self._model.n_ctx(),
        )
