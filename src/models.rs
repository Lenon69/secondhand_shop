// src/models.rs
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "product_condition", rename_all = "PascalCase")]
pub enum ProductCondition {
    New,      // Nowy (np. z metkami, nieużywany)
    LikeNew,  // Jak nowy (użyty minimalnie, bez śladów)
    VeryGood, // Bardzo dobry (niewielkie ślady użytkowania)
    Good,     // Dobry (widoczne ślady użytkowania, ale w pełni funkcjonalny)
    Fair,     // Dostateczny (spore ślady użytkowania, możliwe drobne wady)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "product_status", rename_all = "PascalCase")]
pub enum ProductStatus {
    Available, // Dostępny
    Reserved,  // Zarezerwowany
    Sold,      // Sprzedany
}

// --- NOWY ENUM DLA KATEGORII ---
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "category_type", rename_all = "PascalCase")]
pub enum Category {
    Koszule,          // Shirts
    Spodnie,          // Trousers / Pants
    Sukienki,         // Dresses
    Spodnice,         // Skirts
    Swetry,           // Sweaters / Jumpers
    Bluzy,            // Hoodies / Sweatshirts
    KurtkiPlaszcze,   // Jackets / Coats
    MarynarkiZakiety, // Blazers / Suits
    Obuwie,           // Shoes
    Torebki,          // Bags
    Akcesoria,        // Accessories (paski, czapki, szaliki)
    Bielizna,         // Underwear (jeśli dotyczy)
    StrojeKapielowe,  // Swimwear (jeśli dotyczy)
    Inne,             // Other
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub price: i64,
    pub condition: ProductCondition,
    pub category: Category,
    pub status: ProductStatus,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProductPayload {
    pub name: String,
    pub description: String,
    pub price: i64,
    pub condition: ProductCondition,
    pub category: Category,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProductPayload {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub condition: Option<ProductCondition>,
    pub category: Option<Category>,
    pub status: Option<ProductStatus>,
    pub images: Option<Vec<String>>,
}
