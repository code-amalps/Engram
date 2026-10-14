class DeeplyNestedService:
    def __init__(self, depth: int = 5):
        self.depth = depth

    def get_nesting_level(self) -> int:
        return self.depth
