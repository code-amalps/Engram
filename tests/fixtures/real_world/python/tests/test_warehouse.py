from warehouse.repository import WarehouseRepository
from warehouse.service import WarehouseService

def test_warehouse_receive_and_dispatch():
    repo = WarehouseRepository()
    svc = WarehouseService(repo)
    item = svc.receive_stock("SKU-100", "Widget", 50, "A-12", 4.50)
    assert item.quantity == 50
    assert svc.dispatch_stock("SKU-100", 20) is True
    assert repo.get_by_sku("SKU-100").quantity == 30
