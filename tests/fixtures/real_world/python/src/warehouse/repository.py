from typing import Dict, Optional
from warehouse.models import StockItem

class WarehouseRepository:
    def __init__(self):
        self._storage: Dict[str, StockItem] = {}

    def save(self, item: StockItem) -> None:
        self._storage[item.sku] = item

    def get_by_sku(self, sku: str) -> Optional[StockItem]:
        return self._storage.get(sku)

    def list_all(self):
        return list(self._storage.values())
