function logInfo(msg, meta = {}) {
  const ts = new Date().toISOString();
  console.log(`[INFO] ${ts} - ${msg}`, JSON.stringify(meta));
}

module.exports = { logInfo };
