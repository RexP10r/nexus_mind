import lm_service_pb2_grpc as pb2_grpc

from config import Settings
from server.grpc import GRPCServer
from server.service import LMInferenceService
from core.providers.llama_cpp import LlamaCppProvider


def serve():
    settings = Settings.from_env()

    provider = LlamaCppProvider(settings=settings)

    service = LMInferenceService(provider)

    server = GRPCServer(settings, service)
    server.register(pb2_grpc.add_LMServiceServicer_to_server)
    server.start()
    server.wait()


if __name__ == "__main__":
    serve()
