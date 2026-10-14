from warehouse.models import StockItem
from warehouse.repository import WarehouseRepository

class WarehouseService:
    def __init__(self, repository: WarehouseRepository):
        self.repository = repository

    def receive_stock(self, sku: str, name: str, qty: int, location: str, cost: float) -> StockItem:
        existing = self.repository.get_by_sku(sku)
        if existing:
            existing.quantity += qty
            self.repository.save(existing)
            return existing
        new_item = StockItem(sku=sku, name=name, quantity=qty, bin_location=location, unit_cost=cost)
        self.repository.save(new_item)
        return new_item

    def dispatch_stock(self, sku: str, qty: int) -> bool:
        item = self.repository.get_by_sku(sku)
        if not item or not item.is_available(qty):
            return False
        item.quantity -= qty
        self.repository.save(item)
        return True
