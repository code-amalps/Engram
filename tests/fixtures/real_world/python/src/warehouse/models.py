from dataclasses import dataclass
from typing import Optional

@dataclass
class StockItem:
    sku: str
    name: str
    quantity: int
    bin_location: str
    unit_cost: float

    def is_available(self, requested: int) -> bool:
        return self.quantity >= requested
