// src/models.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Database, Type};
use strum_macros::EnumString;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString)]
#[sqlx(type_name = "product_condition")]
#[strum(ascii_case_insensitive)]
pub enum ProductCondition {
    New,      // Nowy (np. z metkami, nieużywany)
    LikeNew,  // Jak nowy (użyty minimalnie, bez śladów)
    VeryGood, // Bardzo dobry (niewielkie ślady użytkowania)
    Good,     // Dobry (widoczne ślady użytkowania, ale w pełni funkcjonalny)
    Fair,     // Dostateczny (spore ślady użytkowania, możliwe drobne wady)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString)]
#[sqlx(type_name = "product_status")]
#[strum(ascii_case_insensitive)]
pub enum ProductStatus {
    Available, // Dostępny
    Reserved,  // Zarezerwowany
    Sold,      // Sprzedany
}

// --- NOWY ENUM DLA KATEGORII ---
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type, EnumString)]
#[sqlx(type_name = "category_type")]
#[strum(ascii_case_insensitive)]
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

#[allow(dead_code)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
pub enum Role {
    Admin,
    Customer,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    #[sqlx(rename = "email")]
    pub email: String,
    #[sqlx(rename = "password_hash")]
    #[serde(skip_serializing)]
    pub password_hash: String,
    #[sqlx(rename = "role")]
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for UserPublic {
    fn from(user: User) -> Self {
        UserPublic {
            id: user.id,
            email: user.email,
            role: user.role,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

/// Status zamówienia
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[sqlx(type_name = "order_status_enum")]
#[sqlx(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,    // Oczekujące (np. na płatność)
    Processing, // W trakcie realizacji
    Shipped,    // Wysłane
    Delivered,  // Dostarczone
    Cancelled,  // Anulowane
}

/// Reprezentuje pojedyńczą pozycję w zamówieniu
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrderItem {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Uuid,
    pub price_at_purchase: i64,
}

/// Reprezentuje zamówienie
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Validate)]
pub struct Order {
    pub id: Uuid,
    pub user_id: Uuid,
    pub order_date: DateTime<Utc>,
    pub status: OrderStatus,
    pub total_price: i64,

    #[validate(length(min = 1, max = 255))]
    pub shipping_address_line1: String,

    #[validate(length(max = 255))]
    pub shipping_address_line2: String,

    #[validate(length(min = 1, max = 100))]
    pub shipping_city: String,

    #[validate(length(min = 1, max = 20))]
    pub shipping_postal_code: String,

    #[validate(length(min = 1, max = 100))]
    pub shipping_country: String,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// --- STRUKTURY PAYLOAD DLA HANDLERÓW ZAMÓWIEŃ ---

/// Reprezentuje pojedyńczy produkt w payloadzie tworzenia zamówienia
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateOrderPayload {
    #[validate(length(
        min = 1,
        message = "Zamówienie musi zawierać co najmniej jeden produkt"
    ))]
    pub product_ids: Vec<Uuid>,

    #[validate(length(min = 1, max = 255, message = "Linia adresu wysyłki jest wymagana"))]
    pub shipping_address_line1: String,

    #[validate(length(max = 255))]
    pub shipping_address_line2: Option<String>,

    #[validate(length(min = 1, max = 100, message = "Miasto wysyłki jest wymagane"))]
    pub shipping_city: String,

    #[validate(length(min = 1, max = 20, message = "Kod pocztowy wysyłki jest wymagany"))]
    pub shipping_postal_code: String,

    #[validate(length(min = 1, max = 100, message = "Kraj wysyłki jest wymagany"))]
    pub shipping_country: String,
}

/// Payload do aktualizacji statusu zamówienia
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateOrderStatusPayload {
    pub status: OrderStatus,
}

#[derive(Debug, Serialize)]
pub struct OrderDetailsResponse {
    #[serde(flatten)]
    pub order: Order,
    pub items: Vec<OrderItem>,
}

// --- STRUKTURY DLA KOSZYKA ZAKUPÓW ---
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ShoppingCart {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CartItem {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub product_id: Uuid,
    pub added_at: Uuid,
}

// --- STRUKTURY PAYLOAD DLA HANDLERÓW KOSZYKA ---
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AddProductToCartPayload {
    #[validate(required(message = "ID produktu jest wymagane"))]
    pub product_id: Uuid,
}

// --- STRUKTURY ODPOWIEDZI API DLA KOSZYKA ---
// Do wyświetlania pozycji koszyka wraz z danymi produktu
#[derive(Debug, Serialize)]
pub struct CartItemPublic {
    pub cart_item_id: Uuid,
    #[serde(flatten)]
    pub product: Product,
    pub added_at: DateTime<Utc>,
}

pub struct CartDetailsResponse {
    pub cart_id: Uuid,
    pub user_id: Uuid,
    pub items: Vec<CartItemPublic>,
    pub total_items: usize,
    pub total_prize: i64,
    pub updated_at: DateTime<Utc>,
}
