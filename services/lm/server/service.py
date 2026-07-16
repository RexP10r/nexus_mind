import grpc
import lm_service_pb2 as pb2
import lm_service_pb2_grpc as pb2_grpc

from core.provider import LMProvider
from core.types import ChatMessage
from core.exceptions import ModelNotLoadedError, GenerationError


_ROLE_MAP = {
    pb2.ROLE_UNSPECIFIED: "user",
    pb2.ROLE_SYSTEM: "system",
    pb2.ROLE_USER: "user",
    pb2.ROLE_ASSISTANT: "assistant",
}


class LMInferenceService(pb2_grpc.LMServiceServicer):
    def __init__(self, provider: LMProvider):
        self._provider = provider

    def Generate(self, request, context):
        try:
            messages = [
                ChatMessage(
                    role=_ROLE_MAP.get(msg.role, "user"),
                    content=msg.content,
                )
                for msg in request.messages
            ]

            result = self._provider.generate(
                messages=messages,
                temperature=request.temperature,
                max_tokens=request.max_tokens,
                top_p=request.top_p or None,
                top_k=request.top_k or None,
            )

            return pb2.GenerateResponse(
                text=result.text,
                status=pb2.STATUS_SUCCESS,
                tokens_processed=result.tokens_processed,
                tokens_generated=result.tokens_generated,
                duration_ms=result.duration_ms,
            )
        except ModelNotLoadedError as e:
            context.set_code(grpc.StatusCode.FAILED_PRECONDITION)
            context.set_details(str(e))
            return pb2.GenerateResponse(status=pb2.STATUS_ERROR, error_message=str(e))
        except GenerationError as e:
            context.set_code(grpc.StatusCode.INTERNAL)
            context.set_details(str(e))
            return pb2.GenerateResponse(status=pb2.STATUS_ERROR, error_message=str(e))
        except Exception as e:
            context.set_code(grpc.StatusCode.INTERNAL)
            context.set_details(str(e))
            return pb2.GenerateResponse(status=pb2.STATUS_ERROR, error_message=str(e))

    def HealthCheck(self, request, context):
        try:
            info = self._provider.health_check()
            return pb2.HealthCheckResponse(
                is_ready=info.is_ready,
                model_name=info.model_name,
                context_length=info.context_length,
            )
        except Exception as e:
            context.set_code(grpc.StatusCode.INTERNAL)
            context.set_details(str(e))
            return pb2.HealthCheckResponse(is_ready=False)
