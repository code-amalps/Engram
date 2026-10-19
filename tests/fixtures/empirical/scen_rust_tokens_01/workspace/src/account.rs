#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: u64,
    pub balance: i64,
}

impl Account {
    pub fn new(id: u64, balance: i64) -> Self {
        Self { id, balance }
    }
}
