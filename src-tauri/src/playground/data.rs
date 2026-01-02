use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Product data for REST API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Product {
    pub id: u32,
    pub name: String,
    pub price: f64,
    pub category: String,
    pub in_stock: bool,
    pub description: String,
}

/// User data for GraphQL API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: u32,
    pub name: String,
    pub email: String,
    pub created_at: String,
}

/// In-memory data store for playground
#[derive(Debug)]
pub struct PlaygroundData {
    pub products: RwLock<Vec<Product>>,
    pub users: RwLock<Vec<User>>,
    product_id_counter: AtomicU32,
    user_id_counter: AtomicU32,
}

impl PlaygroundData {
    pub fn new() -> Arc<Self> {
        let data = Self {
            products: RwLock::new(Self::seed_products()),
            users: RwLock::new(Self::seed_users()),
            product_id_counter: AtomicU32::new(6), // Start after seeded products
            user_id_counter: AtomicU32::new(6),    // Start after seeded users
        };
        Arc::new(data)
    }

    fn seed_products() -> Vec<Product> {
        vec![
            Product {
                id: 1,
                name: "Wireless Keyboard".to_string(),
                price: 79.99,
                category: "Electronics".to_string(),
                in_stock: true,
                description: "Ergonomic wireless keyboard with backlit keys".to_string(),
            },
            Product {
                id: 2,
                name: "USB-C Hub".to_string(),
                price: 49.99,
                category: "Electronics".to_string(),
                in_stock: true,
                description: "7-in-1 USB-C hub with HDMI and SD card reader".to_string(),
            },
            Product {
                id: 3,
                name: "Standing Desk".to_string(),
                price: 599.00,
                category: "Furniture".to_string(),
                in_stock: false,
                description: "Electric height-adjustable standing desk".to_string(),
            },
            Product {
                id: 4,
                name: "Monitor Light Bar".to_string(),
                price: 89.99,
                category: "Lighting".to_string(),
                in_stock: true,
                description: "LED monitor light bar with auto-dimming".to_string(),
            },
            Product {
                id: 5,
                name: "Mechanical Mouse".to_string(),
                price: 129.99,
                category: "Electronics".to_string(),
                in_stock: true,
                description: "Gaming mouse with customizable buttons".to_string(),
            },
        ]
    }

    fn seed_users() -> Vec<User> {
        vec![
            User {
                id: 1,
                name: "Alice Johnson".to_string(),
                email: "alice@example.com".to_string(),
                created_at: "2024-01-15T10:30:00Z".to_string(),
            },
            User {
                id: 2,
                name: "Bob Smith".to_string(),
                email: "bob@example.com".to_string(),
                created_at: "2024-02-20T14:45:00Z".to_string(),
            },
            User {
                id: 3,
                name: "Carol Williams".to_string(),
                email: "carol@example.com".to_string(),
                created_at: "2024-03-10T09:15:00Z".to_string(),
            },
            User {
                id: 4,
                name: "David Brown".to_string(),
                email: "david@example.com".to_string(),
                created_at: "2024-04-05T16:00:00Z".to_string(),
            },
            User {
                id: 5,
                name: "Eve Davis".to_string(),
                email: "eve@example.com".to_string(),
                created_at: "2024-05-22T11:30:00Z".to_string(),
            },
        ]
    }

    pub fn next_product_id(&self) -> u32 {
        self.product_id_counter.fetch_add(1, Ordering::SeqCst)
    }

    pub fn next_user_id(&self) -> u32 {
        self.user_id_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Reset data to initial state
    pub async fn reset(&self) {
        *self.products.write().await = Self::seed_products();
        *self.users.write().await = Self::seed_users();
        self.product_id_counter.store(6, Ordering::SeqCst);
        self.user_id_counter.store(6, Ordering::SeqCst);
    }
}

impl Default for PlaygroundData {
    fn default() -> Self {
        Self {
            products: RwLock::new(Self::seed_products()),
            users: RwLock::new(Self::seed_users()),
            product_id_counter: AtomicU32::new(6),
            user_id_counter: AtomicU32::new(6),
        }
    }
}
