class LMProviderError(Exception):
    pass


class ModelNotLoadedError(LMProviderError):
    pass


class GenerationError(LMProviderError):
    pass
