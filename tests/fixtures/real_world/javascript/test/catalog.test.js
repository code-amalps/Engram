const assert = require("assert");
const { CatalogManager } = require("../src/catalog");

const mgr = new CatalogManager();
const item = mgr.addItem({ id: "i1", name: "Widget", category: "Hardware" });
assert.strictEqual(item.id, "i1");
assert.strictEqual(mgr.findItemById("i1").name, "Widget");
console.log("JavaScript catalog tests passed.");
