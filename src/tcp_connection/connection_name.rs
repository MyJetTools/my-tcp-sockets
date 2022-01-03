use tokio::sync::RwLock;

pub struct ConnectionName {
    value: RwLock<String>,
}

impl ConnectionName {
    pub fn new(name: String) -> Self {
        Self {
            value: RwLock::new(name),
        }
    }

    pub async fn update(&self, name: String) {
        let mut write_access = self.value.write().await;
        *write_access = name;
    }

    pub async fn get(&self) -> String {
        let read_access = self.value.read().await;
        (*read_access).clone()
    }
}
