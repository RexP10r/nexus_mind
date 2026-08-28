import signal
import sys

import lm_service_pb2_grpc as pb2_grpc

from config import Settings
from server.grpc import GRPCServer
from server.service import LMInferenceService
from core.provider import LMProvider
from core.providers.llama_cpp import LlamaCppProvider


def serve(provider: LMProvider | None = None):
    settings = Settings()

    if provider is None:
        provider = LlamaCppProvider(settings=settings)

    with provider:
        service = LMInferenceService(provider)
        server = GRPCServer(settings, service)
        server.register(pb2_grpc.add_LMServiceServicer_to_server)

        def handler(*_):
            server.stop(grace=30)
            server.wait()
            provider.close()
            sys.exit(0)

        signal.signal(signal.SIGINT, handler)
        signal.signal(signal.SIGTERM, handler)

        server.start()
        server.wait()


if __name__ == "__main__":
    serve()
