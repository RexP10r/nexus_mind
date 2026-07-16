import time
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer

from core.provider import LMProvider
from core.types import ChatMessage, GenerateResult, HealthInfo
from core.exceptions import GenerationError


class TransformersProvider(LMProvider):
    def __init__(
        self,
        model_name: str,
        device: str = "cpu",
        torch_dtype: str = "auto",
    ):
        self._model_name = model_name
        self._device = device
        self._torch_dtype = torch_dtype

        print(f"Loading model {self._model_name} on {self._device}...")
        self._tokenizer = AutoTokenizer.from_pretrained(self._model_name)

        dtype_map = {
            "float16": torch.float16,
            "bfloat16": torch.bfloat16,
            "float32": torch.float32,
        }
        dtype = dtype_map.get(self._torch_dtype, "auto")

        self._model = AutoModelForCausalLM.from_pretrained(
            self._model_name,
            torch_dtype=dtype,
            device_map=None,
        )
        self._model.to(self._device)

        self._model.eval()
        print("Model loaded successfully.")

    def generate(
        self,
        messages: list[ChatMessage],
        temperature: float = 0.7,
        max_tokens: int = 256,
        top_p: float | None = None,
        top_k: float | None = None,
    ) -> GenerateResult:

        text = self._tokenizer.apply_chat_template(
            [{"role": m.role, "content": m.content} for m in messages],
            tokenize=False,
            add_generation_prompt=True,
        )
        inputs, prompt_tokens_count = self._tokenize_input(text)

        gen_kwargs = {
            "max_new_tokens": max_tokens,
            "temperature": temperature,
            "do_sample": temperature > 0,
            "top_p": top_p,
            "top_k": int(top_k) if top_k else None,
        }

        start_time = time.perf_counter()
        try:
            output_ids = self._run_generation(inputs, gen_kwargs)
        except Exception as e:
            raise GenerationError(f"Model generation failed: {e}") from e
        elapsed_ms = (time.perf_counter() - start_time) * 1000

        response_text, generated_count = self._decode_output(
            output_ids, prompt_tokens_count)

        return GenerateResult(
            text=response_text,
            tokens_processed=prompt_tokens_count,
            tokens_generated=generated_count,
            duration_ms=elapsed_ms,
        )

    def _tokenize_input(self, text: str) -> tuple[dict, int]:
        inputs = self._tokenizer(
            text, return_tensors="pt").to(self._model.device)
        prompt_tokens_count = inputs["input_ids"].shape[1]
        return inputs, prompt_tokens_count

    def _run_generation(self, inputs: dict, gen_kwargs: dict) -> torch.Tensor:
        with torch.no_grad():
            return self._model.generate(**inputs, **gen_kwargs)

    def _decode_output(self, output_ids: torch.Tensor, prompt_tokens_count: int) -> tuple[str, int]:
        generated_ids = output_ids[0][prompt_tokens_count:]
        response_text = self._tokenizer.decode(
            generated_ids, skip_special_tokens=True)
        return response_text, len(generated_ids)

    def health_check(self) -> HealthInfo:
        if self._model is None:
            return HealthInfo(is_ready=False)
        return HealthInfo(
            is_ready=True,
            model_name=self._model_name,
            context_length=getattr(
                self._model.config, "max_position_embeddings", 0),
        )
