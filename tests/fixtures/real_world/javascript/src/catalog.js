const { logInfo } = require("./utils/logger");

class CatalogManager {
  constructor() {
    this.items = [];
  }

  addItem(item) {
    this.items.push(item);
    logInfo("Item added to catalog", { id: item.id });
    return item;
  }

  findItemById(id) {
    return this.items.find(i => i.id === id) || null;
  }

  listCategories() {
    const categories = new Set(this.items.map(i => i.category));
    return Array.from(categories);
  }
}

module.exports = { CatalogManager };
