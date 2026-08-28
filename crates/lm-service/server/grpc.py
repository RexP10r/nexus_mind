import grpc
from concurrent import futures

from config import Settings


class GRPCServer:
    def __init__(self, settings: Settings, servicer):
        self._settings = settings
        self._servicer = servicer
        self._server = grpc.server(
            futures.ThreadPoolExecutor(max_workers=settings.max_workers)
        )

    def register(self, add_servicer_fn) -> None:
        add_servicer_fn(self._servicer, self._server)

    def start(self) -> None:
        address = f"{self._settings.host}:{self._settings.port}"
        self._server.add_insecure_port(address)
        self._server.start()
        print(f"gRPC server listening on {address}")

    def stop(self, grace: float | None = 30.0) -> None:
        self._server.stop(grace)

    def wait(self) -> None:
        self._server.wait_for_termination()
