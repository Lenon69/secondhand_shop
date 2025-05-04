// src/models.rs
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "product_condition")]
pub enum ProductCondition {
    New,      // Nowy (np. z metkami, nieużywany)
    LikeNew,  // Jak nowy (użyty minimalnie, bez śladów)
    VeryGood, // Bardzo dobry (niewielkie ślady użytkowania)
    Good,     // Dobry (widoczne ślady użytkowania, ale w pełni funkcjonalny)
    Fair,     // Dostateczny (spore ślady użytkowania, możliwe drobne wady)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "product_status")]
pub enum ProductStatus {
    Available, // Dostępny
    Reserved,  // Zarezerwowany
    Sold,      // Sprzedany
}

// --- NOWY ENUM DLA KATEGORII ---
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "category_type")]
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

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateProductPayload {
    #[validate(length(min = 1, max = 255, message = "Nazwa musi mieć od 1 do 255 znaków"))]
    pub name: String,

    #[validate(length(max = 5000, message = "Opis nie może przekraczać 5000 znaków"))]
    pub description: String,

    #[validate(range(min = 0, message = "Cena nie może być ujemna"))]
    pub price: i64,

    pub condition: ProductCondition,
    pub category: Category,
    #[validate(length(min = 1, message = "Należy dodać przynajmniej jeden URL obrazka"))]
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateProductPayload {
    #[validate(length(min = 1, max = 255, message = "Nazwa musi mieć od 1 do 255 znaków"))]
    pub name: Option<String>,

    #[validate(length(max = 5000, message = "Opis nie może przekraczać 5000 znaków"))]
    pub description: Option<String>,

    #[validate(range(min = 0, message = "Cena nie może być ujemna"))]
    pub price: Option<i64>,

    pub condition: Option<ProductCondition>,
    pub category: Option<Category>,
    pub status: Option<ProductStatus>,

    #[validate(length(min = 1, message = "Należy dodać przynajmniej jeden URL obrazka"))]
    pub images: Option<Vec<String>>,
}
