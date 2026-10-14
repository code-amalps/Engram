function setupRoutes(app, catalogManager) {
  app.get("/items", (req, res) => {
    res.json(catalogManager.items);
  });

  app.post("/items", (req, res) => {
    const item = catalogManager.addItem(req.body);
    res.status(201).json(item);
  });
}

module.exports = { setupRoutes };
